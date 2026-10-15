"""Traces → training data → (on a GPU machine) a LoRA for the creature.

The ingestion half of Pattern N already runs: the watcher folds the
creature's exchanges into the user model as preference items. This module
is the other half — turning those traces into a training dataset, and, when
the hardware allows, a fine-tuned adapter.

Honesty about the split, because the two halves have very different
prerequisites:

- The dataset pipeline runs anywhere. It pulls the user model's approved
  items, formats them as JSONL instruction data, and writes a real file.
  That artifact is verifiable on this machine today.
- Neural training needs an NVIDIA GPU and unsloth. Neither is present on
  the development host (no CUDA device, unsloth not installed), so the
  training path is real code guarded by an availability check — it runs
  where the hardware exists and reports exactly what is missing where it
  does not. It never claims a success it did not produce.
"""
from __future__ import annotations

import json
import os
import time
from typing import Any, Optional

DATASET_DIRNAME = "finetune"
BASE_MODEL = "unsloth/qwen2.5-0.5b-instruct-bnb-4bit"
MAX_ITEMS = 200


def dataset_dir() -> str:
    """Where datasets and adapters live: <ABP_PRIVATE_DIR>/finetune."""
    root = None
    try:
        from bot import extensions as _ext

        root = _ext.private_dir(os.environ)
    except Exception:  # noqa: BLE001 - no private dir is the public default
        root = None
    path = os.path.join(root, DATASET_DIRNAME) if root else os.path.join(
        os.path.dirname(os.path.abspath(__file__)), DATASET_DIRNAME
    )
    os.makedirs(path, exist_ok=True)
    return path


def _creature_instance():
    from bot import bot_instances

    try:
        instances = bot_instances.list_instances()
    except Exception:  # noqa: BLE001 - no inventory, no dataset
        return None
    return next((i for i in instances if i.get("name") == "creature"), None)


def build_dataset(limit: int = MAX_ITEMS) -> dict:
    """The traces → dataset half. Runs anywhere; writes a real JSONL file.

    Each approved user-model item becomes one instruction example: the
    kind (preference/goal/...) is the instruction, the item's text the
    demonstration. The file is the artifact a trainer consumes.
    """
    from bot.agent_runtime import user_model

    inst = _creature_instance()
    if inst is None:
        return {"ok": False, "error": "no 'creature' instance", "path": None,
                "items": 0, "examples": 0}
    try:
        items = user_model.list_items(inst["id"], "creature", status="approved")
    except TypeError:
        # A build without the status filter takes every item.
        items = user_model.list_items(inst["id"], "creature")
    except Exception as exc:  # noqa: BLE001 - an unreadable model yields an honest empty
        return {"ok": False, "error": f"user model unreadable: {exc}",
                "path": None, "items": 0, "examples": 0}
    items = items[:limit]
    if not items:
        return {"ok": True, "path": None, "items": 0, "examples": 0,
                "note": "no approved items yet — the watcher ingests exchanges as they happen"}

    examples = []
    for it in items:
        kind = str(it.get("kind") or "preference")
        text = str(it.get("text") or "").strip()
        if not text:
            continue
        examples.append({
            "messages": [
                {"role": "user", "content": f"The user's {kind} is: {text}"},
                {"role": "assistant", "content": text},
            ]
        })
    if not examples:
        return {"ok": True, "path": None, "items": len(items), "examples": 0,
                "note": "approved items carried no usable text"}

    path = os.path.join(dataset_dir(), f"creature-{int(time.time())}.jsonl")
    with open(path, "w", encoding="utf-8") as f:
        for ex in examples:
            f.write(json.dumps(ex, ensure_ascii=False) + "\n")
    return {"ok": True, "path": path, "items": len(items),
            "examples": len(examples)}


def backend_status() -> dict:
    """What training hardware this machine actually has — never a guess."""
    try:
        import torch
    except Exception:  # noqa: BLE001
        return {"trainable": False, "reason": "torch is not installed"}
    if not torch.cuda.is_available():
        return {"trainable": False,
                "reason": "no CUDA device (fine-tuning a 0.5B model on CPU is not practical)"}
    try:
        import unsloth  # noqa: F401
    except Exception:  # noqa: BLE001
        return {"trainable": False, "reason": "torch+CUDA present but unsloth is not installed"}
    return {"trainable": True, "reason": f"unsloth on {torch.cuda.get_device_name(0)}"}


def train_adapter(dataset_path: str, out_dir: Optional[str] = None) -> dict:
    """The training half — real unsloth LoRA code, guarded by availability.

    Runs only where backend_status() is trainable. Returns a dict that
    always states which branch executed; a skipped training is reported
    as skipped, never as a silent no-op or a false success.
    """
    backend = backend_status()
    if not backend.get("trainable"):
        return {"ok": False, "trained": False, "skipped": True,
                "reason": backend.get("reason"), "adapter": None}
    if not dataset_path or not os.path.isfile(dataset_path):
        return {"ok": False, "trained": False, "skipped": True,
                "reason": "no dataset file", "adapter": None}
    try:
        from unsloth import FastLanguageModel  # noqa: F401
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "trained": False, "skipped": True,
                "reason": f"unsloth import failed: {exc}", "adapter": None}

    out_dir = out_dir or os.path.join(dataset_dir(), "adapter")
    os.makedirs(out_dir, exist_ok=True)
    # The training itself: LoRA on the creature's instruct base, the
    # dataset's instruction format, one epoch — a smoke-real fine-tune,
    # not a placeholder. Runs only on the guarded path above.
    from unsloth import FastLanguageModel

    model, tokenizer = FastLanguageModel.from_pretrained(
        BASE_MODEL, max_seq_length=1024, load_in_4bit=True,
    )
    model = FastLanguageModel.get_peft_model(
        model, r=16, lora_alpha=16, lora_dropout=0,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj",
                        "gate_proj", "up_proj", "down_proj"],
    )
    from trl import SFTTrainer, SFTConfig  # noqa: F401

    trainer = SFTTrainer(
        model=model, tokenizer=tokenizer,
        train_dataset=_load_jsonl(dataset_path),
        args=SFTConfig(
            per_device_train_batch_size=2, gradient_accumulation_steps=4,
            num_train_epochs=1, learning_rate=2e-4, output_dir=out_dir,
        ),
    )
    trainer.train()
    model.save_pretrained(out_dir)
    tokenizer.save_pretrained(out_dir)
    return {"ok": True, "trained": True, "skipped": False,
            "reason": "unsloth LoRA trained", "adapter": out_dir,
            "base": BASE_MODEL}


def _load_jsonl(path: str):
    """The dataset as a TRL-compatible dataset, built from the JSONL file."""
    from datasets import load_dataset

    return load_dataset("json", data_files=path, split="train")


def status() -> dict:
    """The full state: dataset artifacts, backend, adapter."""
    d = dataset_dir()
    datasets = []
    try:
        for name in sorted(os.listdir(d)):
            full = os.path.join(d, name)
            if os.path.isfile(full):
                datasets.append({"name": name, "bytes": os.path.getsize(full)})
    except OSError:
        pass
    adapter = os.path.join(d, "adapter")
    return {
        "dir": d,
        "datasets": datasets,
        "backend": backend_status(),
        "adapter": adapter if os.path.isdir(adapter) else None,
        "base_model": BASE_MODEL,
    }
