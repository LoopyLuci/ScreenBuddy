package com.screenbuddy.android.data.model

object ProviderData {
    val providers = listOf(
        Provider(
            id = "ollama",
            name = "Ollama",
            description = "Run open-source models locally on your device or server",
            apiKeyName = "OLLAMA_BASE_URL",
            website = "https://ollama.com",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "ollama-llama3.2",
                    providerId = "ollama",
                    name = "llama3.2",
                    displayName = "Llama 3.2",
                    isFree = true,
                    isEnabled = true,
                    description = "Meta's latest Llama model, optimized for local inference",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "ollama-llama3.2-vision",
                    providerId = "ollama",
                    name = "llama3.2-vision",
                    displayName = "Llama 3.2 Vision",
                    isFree = true,
                    isEnabled = true,
                    description = "Multimodal model with vision capabilities",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "ollama-mistral",
                    providerId = "ollama",
                    name = "mistral",
                    displayName = "Mistral 7B",
                    isFree = true,
                    isEnabled = true,
                    description = "Fast and efficient open-source model",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "ollama-codellama",
                    providerId = "ollama",
                    name = "codellama",
                    displayName = "Code Llama",
                    isFree = true,
                    isEnabled = true,
                    description = "Specialized for code generation and completion",
                    maxTokens = 16384
                ),
                AiModel(
                    id = "ollama-gemma2",
                    providerId = "ollama",
                    name = "gemma2",
                    displayName = "Gemma 2",
                    isFree = true,
                    isEnabled = true,
                    description = "Google's open-weight model",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "ollama-qwen2.5",
                    providerId = "ollama",
                    name = "qwen2.5",
                    displayName = "Qwen 2.5",
                    isFree = true,
                    isEnabled = true,
                    description = "Alibaba's powerful open-source model",
                    maxTokens = 32768
                ),
                AiModel(
                    id = "ollama-phi3",
                    providerId = "ollama",
                    name = "phi3",
                    displayName = "Phi-3",
                    isFree = true,
                    isEnabled = true,
                    description = "Microsoft's compact and capable model",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "ollama-neural-chat",
                    providerId = "ollama",
                    name = "neural-chat",
                    displayName = "Neural Chat",
                    isFree = true,
                    isEnabled = true,
                    description = "Intel's conversational AI model",
                    maxTokens = 4096
                )
            )
        ),
        Provider(
            id = "openai",
            name = "OpenAI",
            description = "Industry-leading AI models from OpenAI",
            apiKeyName = "OPENAI_API_KEY",
            website = "https://openai.com",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "openai-gpt-4o-mini",
                    providerId = "openai",
                    name = "gpt-4o-mini",
                    displayName = "GPT-4o Mini",
                    isFree = true,
                    isEnabled = true,
                    description = "Affordable and capable small model",
                    maxTokens = 16384,
                    costPer1k = 0.15
                ),
                AiModel(
                    id = "openai-gpt-4o",
                    providerId = "openai",
                    name = "gpt-4o",
                    displayName = "GPT-4o",
                    isFree = false,
                    isEnabled = false,
                    description = "Most capable multimodal model",
                    maxTokens = 128000,
                    costPer1k = 5.0
                ),
                AiModel(
                    id = "openai-gpt-4-turbo",
                    providerId = "openai",
                    name = "gpt-4-turbo",
                    displayName = "GPT-4 Turbo",
                    isFree = false,
                    isEnabled = false,
                    description = "High-performance GPT-4 variant",
                    maxTokens = 128000,
                    costPer1k = 10.0
                ),
                AiModel(
                    id = "openai-gpt-3.5-turbo",
                    providerId = "openai",
                    name = "gpt-3.5-turbo",
                    displayName = "GPT-3.5 Turbo",
                    isFree = false,
                    isEnabled = false,
                    description = "Fast and affordable chat model",
                    maxTokens = 16384,
                    costPer1k = 0.5
                ),
                AiModel(
                    id = "openai-o1-preview",
                    providerId = "openai",
                    name = "o1-preview",
                    displayName = "o1 Preview",
                    isFree = false,
                    isEnabled = false,
                    description = "Advanced reasoning model",
                    maxTokens = 128000,
                    costPer1k = 15.0
                ),
                AiModel(
                    id = "openai-o1-mini",
                    providerId = "openai",
                    name = "o1-mini",
                    displayName = "o1 Mini",
                    isFree = false,
                    isEnabled = false,
                    description = "Compact reasoning model",
                    maxTokens = 128000,
                    costPer1k = 3.0
                )
            )
        ),
        Provider(
            id = "anthropic",
            name = "Anthropic",
            description = "Claude models with strong reasoning and safety",
            apiKeyName = "ANTHROPIC_API_KEY",
            website = "https://anthropic.com",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "anthropic-claude-3.5-haiku",
                    providerId = "anthropic",
                    name = "claude-3-5-haiku-20241022",
                    displayName = "Claude 3.5 Haiku",
                    isFree = true,
                    isEnabled = true,
                    description = "Fastest and most affordable Claude model",
                    maxTokens = 8192,
                    costPer1k = 0.25
                ),
                AiModel(
                    id = "anthropic-claude-3.5-sonnet",
                    providerId = "anthropic",
                    name = "claude-3-5-sonnet-20241022",
                    displayName = "Claude 3.5 Sonnet",
                    isFree = false,
                    isEnabled = false,
                    description = "Balanced intelligence and speed",
                    maxTokens = 8192,
                    costPer1k = 3.0
                ),
                AiModel(
                    id = "anthropic-claude-3-opus",
                    providerId = "anthropic",
                    name = "claude-3-opus-20240229",
                    displayName = "Claude 3 Opus",
                    isFree = false,
                    isEnabled = false,
                    description = "Most powerful Claude model",
                    maxTokens = 4096,
                    costPer1k = 15.0
                ),
                AiModel(
                    id = "anthropic-claude-3-sonnet",
                    providerId = "anthropic",
                    name = "claude-3-sonnet-20240229",
                    displayName = "Claude 3 Sonnet",
                    isFree = false,
                    isEnabled = false,
                    description = "Balanced Claude 3 model",
                    maxTokens = 4096,
                    costPer1k = 3.0
                )
            )
        ),
        Provider(
            id = "openrouter",
            name = "OpenRouter",
            description = "Access to 100+ models through a single API",
            apiKeyName = "OPENROUTER_API_KEY",
            website = "https://openrouter.ai",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "or-mistral-7b",
                    providerId = "openrouter",
                    name = "mistralai/mistral-7b-instruct:free",
                    displayName = "Mistral 7B (Free)",
                    isFree = true,
                    isEnabled = true,
                    description = "Free Mistral 7B via OpenRouter",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "or-llama-3.1-8b",
                    providerId = "openrouter",
                    name = "meta-llama/llama-3.1-8b-instruct:free",
                    displayName = "Llama 3.1 8B (Free)",
                    isFree = true,
                    isEnabled = true,
                    description = "Free Llama 3.1 8B via OpenRouter",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "or-gemma-2-9b",
                    providerId = "openrouter",
                    name = "google/gemma-2-9b-it:free",
                    displayName = "Gemma 2 9B (Free)",
                    isFree = true,
                    isEnabled = true,
                    description = "Free Gemma 2 9B via OpenRouter",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "or-phi-3-medium",
                    providerId = "openrouter",
                    name = "microsoft/phi-3-medium-128k-instruct:free",
                    displayName = "Phi-3 Medium (Free)",
                    isFree = true,
                    isEnabled = true,
                    description = "Free Phi-3 Medium via OpenRouter",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "or-claude-3.5-sonnet",
                    providerId = "openrouter",
                    name = "anthropic/claude-3.5-sonnet",
                    displayName = "Claude 3.5 Sonnet",
                    isFree = false,
                    isEnabled = false,
                    description = "Premium Claude via OpenRouter",
                    maxTokens = 8192,
                    costPer1k = 3.0
                ),
                AiModel(
                    id = "or-gpt-4o",
                    providerId = "openrouter",
                    name = "openai/gpt-4o",
                    displayName = "GPT-4o",
                    isFree = false,
                    isEnabled = false,
                    description = "Premium GPT-4o via OpenRouter",
                    maxTokens = 128000,
                    costPer1k = 5.0
                )
            )
        ),
        Provider(
            id = "together",
            name = "Together AI",
            description = "Fast inference for open-source models",
            apiKeyName = "TOGETHER_API_KEY",
            website = "https://together.ai",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "together-llama-3.1-8b",
                    providerId = "together",
                    name = "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo",
                    displayName = "Llama 3.1 8B Turbo",
                    isFree = true,
                    isEnabled = true,
                    description = "Fast Llama 3.1 8B inference",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "together-llama-3.1-70b",
                    providerId = "together",
                    name = "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
                    displayName = "Llama 3.1 70B Turbo",
                    isFree = false,
                    isEnabled = false,
                    description = "Powerful Llama 3.1 70B model",
                    maxTokens = 8192,
                    costPer1k = 0.88
                ),
                AiModel(
                    id = "together-mistral-7b",
                    providerId = "together",
                    name = "mistralai/Mistral-7B-Instruct-v0.3",
                    displayName = "Mistral 7B v0.3",
                    isFree = true,
                    isEnabled = true,
                    description = "Mistral 7B instruction model",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "together-qwen-2.5-72b",
                    providerId = "together",
                    name = "Qwen/Qwen2.5-72B-Instruct-Turbo",
                    displayName = "Qwen 2.5 72B Turbo",
                    isFree = false,
                    isEnabled = false,
                    description = "Powerful Qwen 2.5 72B model",
                    maxTokens = 32768,
                    costPer1k = 0.88
                ),
                AiModel(
                    id = "together-deepseek-v3",
                    providerId = "together",
                    name = "deepseek-ai/DeepSeek-V3",
                    displayName = "DeepSeek V3",
                    isFree = false,
                    isEnabled = false,
                    description = "Advanced DeepSeek model",
                    maxTokens = 32768,
                    costPer1k = 1.25
                )
            )
        ),
        Provider(
            id = "deepseek",
            name = "DeepSeek",
            description = "Powerful Chinese AI models with competitive pricing",
            apiKeyName = "DEEPSEEK_API_KEY",
            website = "https://deepseek.com",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "deepseek-chat",
                    providerId = "deepseek",
                    name = "deepseek-chat",
                    displayName = "DeepSeek Chat",
                    isFree = true,
                    isEnabled = true,
                    description = "General purpose chat model",
                    maxTokens = 8192,
                    costPer1k = 0.07
                ),
                AiModel(
                    id = "deepseek-coder",
                    providerId = "deepseek",
                    name = "deepseek-coder",
                    displayName = "DeepSeek Coder",
                    isFree = true,
                    isEnabled = true,
                    description = "Specialized for code generation",
                    maxTokens = 16384,
                    costPer1k = 0.07
                ),
                AiModel(
                    id = "deepseek-reasoner",
                    providerId = "deepseek",
                    name = "deepseek-reasoner",
                    displayName = "DeepSeek Reasoner",
                    isFree = false,
                    isEnabled = false,
                    description = "Advanced reasoning model",
                    maxTokens = 32768,
                    costPer1k = 0.55
                )
            )
        ),
        Provider(
            id = "mistral",
            name = "Mistral AI",
            description = "European AI company with efficient models",
            apiKeyName = "MISTRAL_API_KEY",
            website = "https://mistral.ai",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "mistral-open-mistral-7b",
                    providerId = "mistral",
                    name = "open-mistral-7b",
                    displayName = "Open Mistral 7B",
                    isFree = true,
                    isEnabled = true,
                    description = "Open-source Mistral 7B model",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "mistral-open-mixtral-8x7b",
                    providerId = "mistral",
                    name = "open-mixtral-8x7b",
                    displayName = "Open Mixtral 8x7B",
                    isFree = true,
                    isEnabled = true,
                    description = "Open-source Mixtral MoE model",
                    maxTokens = 32768
                ),
                AiModel(
                    id = "mistral-small-latest",
                    providerId = "mistral",
                    name = "mistral-small-latest",
                    displayName = "Mistral Small",
                    isFree = false,
                    isEnabled = false,
                    description = "Affordable small model",
                    maxTokens = 32768,
                    costPer1k = 0.2
                ),
                AiModel(
                    id = "mistral-medium-latest",
                    providerId = "mistral",
                    name = "mistral-medium-latest",
                    displayName = "Mistral Medium",
                    isFree = false,
                    isEnabled = false,
                    description = "Balanced medium model",
                    maxTokens = 32768,
                    costPer1k = 2.7
                ),
                AiModel(
                    id = "mistral-large-latest",
                    providerId = "mistral",
                    name = "mistral-large-latest",
                    displayName = "Mistral Large",
                    isFree = false,
                    isEnabled = false,
                    description = "Most capable Mistral model",
                    maxTokens = 128000,
                    costPer1k = 8.0
                )
            )
        ),
        Provider(
            id = "cohere",
            name = "Cohere",
            description = "Enterprise-focused language models",
            apiKeyName = "COHERE_API_KEY",
            website = "https://cohere.com",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "cohere-command-r",
                    providerId = "cohere",
                    name = "command-r",
                    displayName = "Command R",
                    isFree = true,
                    isEnabled = true,
                    description = "Retrieval-augmented generation model",
                    maxTokens = 4096,
                    costPer1k = 0.5
                ),
                AiModel(
                    id = "cohere-command-r-plus",
                    providerId = "cohere",
                    name = "command-r-plus",
                    displayName = "Command R+",
                    isFree = false,
                    isEnabled = false,
                    description = "Enhanced RAG model",
                    maxTokens = 4096,
                    costPer1k = 3.0
                ),
                AiModel(
                    id = "cohere-command-light",
                    providerId = "cohere",
                    name = "command-light",
                    displayName = "Command Light",
                    isFree = true,
                    isEnabled = true,
                    description = "Lightweight and fast model",
                    maxTokens = 4096,
                    costPer1k = 0.3
                )
            )
        ),
        Provider(
            id = "perplexity",
            name = "Perplexity",
            description = "AI models with real-time web search",
            apiKeyName = "PERPLEXITY_API_KEY",
            website = "https://perplexity.ai",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "pplx-sonar-small",
                    providerId = "perplexity",
                    name = "llama-3.1-sonar-small-128k-online",
                    displayName = "Sonar Small",
                    isFree = true,
                    isEnabled = true,
                    description = "Small model with web search",
                    maxTokens = 128000,
                    costPer1k = 0.2
                ),
                AiModel(
                    id = "pplx-sonar-large",
                    providerId = "perplexity",
                    name = "llama-3.1-sonar-large-128k-online",
                    displayName = "Sonar Large",
                    isFree = false,
                    isEnabled = false,
                    description = "Large model with web search",
                    maxTokens = 128000,
                    costPer1k = 1.0
                ),
                AiModel(
                    id = "pplx-sonar-huge",
                    providerId = "perplexity",
                    name = "llama-3.1-sonar-huge-128k-online",
                    displayName = "Sonar Huge",
                    isFree = false,
                    isEnabled = false,
                    description = "Largest model with web search",
                    maxTokens = 128000,
                    costPer1k = 5.0
                )
            )
        ),
        Provider(
            id = "replicate",
            name = "Replicate",
            description = "Run any ML model with a cloud API",
            apiKeyName = "REPLICATE_API_TOKEN",
            website = "https://replicate.com",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "rep-llama-3.1-405b",
                    providerId = "replicate",
                    name = "meta/meta-llama-3.1-405b-instruct",
                    displayName = "Llama 3.1 405B",
                    isFree = false,
                    isEnabled = false,
                    description = "Massive Llama model on Replicate",
                    maxTokens = 16384,
                    costPer1k = 9.5
                ),
                AiModel(
                    id = "rep-mixtral-8x7b",
                    providerId = "replicate",
                    name = "mistralai/mixtral-8x7b-instruct-v0.1",
                    displayName = "Mixtral 8x7B",
                    isFree = false,
                    isEnabled = false,
                    description = "Mixtral MoE on Replicate",
                    maxTokens = 32768,
                    costPer1k = 0.6
                ),
                AiModel(
                    id = "rep-llama-3-70b",
                    providerId = "replicate",
                    name = "meta/meta-llama-3-70b-instruct",
                    displayName = "Llama 3 70B",
                    isFree = false,
                    isEnabled = false,
                    description = "Llama 3 70B on Replicate",
                    maxTokens = 8192,
                    costPer1k = 0.65
                )
            )
        ),
        Provider(
            id = "huggingface",
            name = "Hugging Face",
            description = "The AI community platform with thousands of models",
            apiKeyName = "HUGGINGFACE_API_KEY",
            website = "https://huggingface.co",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "hf-llama-3.1-8b",
                    providerId = "huggingface",
                    name = "meta-llama/Llama-3.1-8B-Instruct",
                    displayName = "Llama 3.1 8B",
                    isFree = true,
                    isEnabled = true,
                    description = "Free Llama 3.1 8B inference",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "hf-mistral-7b",
                    providerId = "huggingface",
                    name = "mistralai/Mistral-7B-Instruct-v0.3",
                    displayName = "Mistral 7B",
                    isFree = true,
                    isEnabled = true,
                    description = "Free Mistral 7B inference",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "hf-phi-3-mini",
                    providerId = "huggingface",
                    name = "microsoft/Phi-3-mini-4k-instruct",
                    displayName = "Phi-3 Mini",
                    isFree = true,
                    isEnabled = true,
                    description = "Free Phi-3 Mini inference",
                    maxTokens = 4096
                ),
                AiModel(
                    id = "hf-qwen-2.5-72b",
                    providerId = "huggingface",
                    name = "Qwen/Qwen2.5-72B-Instruct",
                    displayName = "Qwen 2.5 72B",
                    isFree = false,
                    isEnabled = false,
                    description = "Premium Qwen 2.5 72B",
                    maxTokens = 32768,
                    costPer1k = 0.9
                )
            )
        ),
        Provider(
            id = "google",
            name = "Google AI",
            description = "Gemini models from Google",
            apiKeyName = "GOOGLE_API_KEY",
            website = "https://ai.google.dev",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "gemini-1.5-flash",
                    providerId = "google",
                    name = "gemini-1.5-flash",
                    displayName = "Gemini 1.5 Flash",
                    isFree = true,
                    isEnabled = true,
                    description = "Fast and efficient Gemini model",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "gemini-1.5-flash-8b",
                    providerId = "google",
                    name = "gemini-1.5-flash-8b",
                    displayName = "Gemini 1.5 Flash 8B",
                    isFree = true,
                    isEnabled = true,
                    description = "Smallest Gemini Flash model",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "gemini-1.5-pro",
                    providerId = "google",
                    name = "gemini-1.5-pro",
                    displayName = "Gemini 1.5 Pro",
                    isFree = false,
                    isEnabled = false,
                    description = "Most capable Gemini model",
                    maxTokens = 32768,
                    costPer1k = 1.25
                ),
                AiModel(
                    id = "gemini-1.0-pro",
                    providerId = "google",
                    name = "gemini-1.0-pro",
                    displayName = "Gemini 1.0 Pro",
                    isFree = false,
                    isEnabled = false,
                    description = "Previous generation Gemini Pro",
                    maxTokens = 32768,
                    costPer1k = 0.5
                )
            )
        ),
        Provider(
            id = "grok",
            name = "Grok (xAI)",
            description = "AI models from xAI with real-time knowledge",
            apiKeyName = "XAI_API_KEY",
            website = "https://x.ai",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "grok-beta",
                    providerId = "grok",
                    name = "grok-beta",
                    displayName = "Grok Beta",
                    isFree = true,
                    isEnabled = true,
                    description = "Beta access to Grok model",
                    maxTokens = 131072,
                    costPer1k = 0.2
                ),
                AiModel(
                    id = "grok-2",
                    providerId = "grok",
                    name = "grok-2",
                    displayName = "Grok 2",
                    isFree = false,
                    isEnabled = false,
                    description = "Latest Grok model",
                    maxTokens = 131072,
                    costPer1k = 2.0
                ),
                AiModel(
                    id = "grok-2-vision",
                    providerId = "grok",
                    name = "grok-2-vision",
                    displayName = "Grok 2 Vision",
                    isFree = false,
                    isEnabled = false,
                    description = "Multimodal Grok model",
                    maxTokens = 131072,
                    costPer1k = 2.0
                )
            )
        ),
        Provider(
            id = "unsloth",
            name = "Unsloth",
            description = "Fine-tuned models optimized for speed",
            apiKeyName = "UNSLOTH_API_KEY",
            website = "https://unsloth.ai",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "unsloth-llama-3.1-8b",
                    providerId = "unsloth",
                    name = "unsloth/Llama-3.1-8B-Instruct",
                    displayName = "Llama 3.1 8B (Unsloth)",
                    isFree = true,
                    isEnabled = true,
                    description = "Optimized Llama 3.1 8B",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "unsloth-mistral-7b",
                    providerId = "unsloth",
                    name = "unsloth/Mistral-7B-Instruct-v0.3",
                    displayName = "Mistral 7B (Unsloth)",
                    isFree = true,
                    isEnabled = true,
                    description = "Optimized Mistral 7B",
                    maxTokens = 8192
                ),
                AiModel(
                    id = "unsloth-gemma-2-9b",
                    providerId = "unsloth",
                    name = "unsloth/gemma-2-9b-it",
                    displayName = "Gemma 2 9B (Unsloth)",
                    isFree = true,
                    isEnabled = true,
                    description = "Optimized Gemma 2 9B",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "llmstudio",
            name = "LLM Studio",
            description = "Desktop app for running local models",
            apiKeyName = "LLM_STUDIO_URL",
            website = "https://lmstudio.ai",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "llmstudio-local",
                    providerId = "llmstudio",
                    name = "local-model",
                    displayName = "Local Model (LLM Studio)",
                    isFree = true,
                    isEnabled = true,
                    description = "Any model running in LLM Studio",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "vllm",
            name = "vLLM",
            description = "High-throughput LLM serving engine",
            apiKeyName = "VLLM_URL",
            website = "https://vllm.ai",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "vllm-local",
                    providerId = "vllm",
                    name = "local-model",
                    displayName = "Local Model (vLLM)",
                    isFree = true,
                    isEnabled = true,
                    description = "Any model served via vLLM",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "llamacpp",
            name = "llama.cpp",
            description = "C++ implementation for running LLaMA models",
            apiKeyName = "LLAMACPP_URL",
            website = "https://github.com/ggerganov/llama.cpp",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "llamacpp-local",
                    providerId = "llamacpp",
                    name = "local-model",
                    displayName = "Local Model (llama.cpp)",
                    isFree = true,
                    isEnabled = true,
                    description = "Any model running via llama.cpp",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "lmstudio",
            name = "LM Studio",
            description = "Desktop app for discovering and running LLMs",
            apiKeyName = "LM_STUDIO_URL",
            website = "https://lmstudio.ai",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "lmstudio-local",
                    providerId = "lmstudio",
                    name = "local-model",
                    displayName = "Local Model (LM Studio)",
                    isFree = true,
                    isEnabled = true,
                    description = "Any model running in LM Studio",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "textgen",
            name = "Text Generation WebUI",
            description = "Popular web UI for text generation models",
            apiKeyName = "TEXTGEN_URL",
            website = "https://github.com/oobabooga/text-generation-webui",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "textgen-local",
                    providerId = "textgen",
                    name = "local-model",
                    displayName = "Local Model (TextGen)",
                    isFree = true,
                    isEnabled = true,
                    description = "Any model running in TextGen WebUI",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "koboldcpp",
            name = "Koboldcpp",
            description = "Lightweight llama.cpp wrapper for gaming laptops",
            apiKeyName = "KOBOLDCPP_URL",
            website = "https://github.com/LostRuins/koboldcpp",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "koboldcpp-local",
                    providerId = "koboldcpp",
                    name = "local-model",
                    displayName = "Local Model (Koboldcpp)",
                    isFree = true,
                    isEnabled = true,
                    description = "Any model running via Koboldcpp",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "opencode-go",
            name = "OpenCode Go",
            description = "Open-source coding assistant (Go implementation)",
            apiKeyName = "OPENCODE_GO_URL",
            website = "https://github.com/opencode-ai/opencode-go",
            isFree = true,
            models = listOf(
                AiModel(
                    id = "opencode-go-local",
                    providerId = "opencode-go",
                    name = "local-model",
                    displayName = "Local Model (OpenCode Go)",
                    isFree = true,
                    isEnabled = true,
                    description = "Any model running via OpenCode Go",
                    maxTokens = 8192
                )
            )
        ),
        Provider(
            id = "opencode-zen",
            name = "OpenCode Zen",
            description = "Premium tier of OpenCode with enhanced features",
            apiKeyName = "OPENCODE_ZEN_API_KEY",
            website = "https://opencode.ai",
            isFree = false,
            models = listOf(
                AiModel(
                    id = "opencode-zen-default",
                    providerId = "opencode-zen",
                    name = "zen-default",
                    displayName = "Zen Default",
                    isFree = true,
                    isEnabled = true,
                    description = "Default Zen model",
                    maxTokens = 8192,
                    costPer1k = 0.1
                ),
                AiModel(
                    id = "opencode-zen-pro",
                    providerId = "opencode-zen",
                    name = "zen-pro",
                    displayName = "Zen Pro",
                    isFree = false,
                    isEnabled = false,
                    description = "Premium Zen model",
                    maxTokens = 32768,
                    costPer1k = 2.0
                )
            )
        )
    )
}