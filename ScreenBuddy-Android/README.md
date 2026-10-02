# ScreenBuddy Android

A production-grade Android virtual assistant app built with Kotlin and Jetpack Compose.

## Features

- **AI Chat**: Chat with 20+ AI providers (Ollama, OpenAI, Anthropic, etc.)
- **Provider Management**: Easy API key setup, model toggles, free/paid separation
- **Creature Selection**: 7 animated companions with unique personalities
- **Settings**: Comprehensive configuration for all features
- **RAG Memory**: Document ingestion and semantic search
- **Agent Runtime**: Tool-using AI with move, sound, animation, notification tools
- **Text-to-Speech**: Speak AI responses aloud
- **System Integration**: Idle detection, activity monitoring

## Architecture

- **UI**: Jetpack Compose with Material 3
- **Database**: Room for local persistence
- **Networking**: OkHttp + Retrofit
- **State**: ViewModel + StateFlow
- **Navigation**: Compose Navigation

## Building

```bash
./gradlew assembleDebug
```

## Project Structure

```
app/src/main/java/com/screenbuddy/android/
├── data/
│   ├── local/          # Room database, DAOs, entities
│   ├── model/          # Data classes (Provider, AiModel, etc.)
│   ├── remote/         # API services
│   └── repository/     # Data repositories
├── ui/
│   ├── screens/        # Compose screens
│   ├── components/     # Reusable UI components
│   └── theme/          # Material 3 theme
├── viewmodel/          # ViewModels
├── service/            # Background services
└── receiver/           # Broadcast receivers
```

## Providers Supported

- Ollama (local)
- OpenAI
- Anthropic
- OpenRouter
- Together AI
- DeepSeek
- Mistral AI
- Cohere
- Perplexity
- Replicate
- Hugging Face
- Google (Gemini)
- Grok (xAI)
- Unsloth
- LLM Studio
- vLLM
- llama.cpp
- LM Studio
- Text Generation WebUI
- Koboldcpp
- OpenCode Go
- OpenCode Zen

## License

MIT
