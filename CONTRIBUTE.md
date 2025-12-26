# Contributing to GuardRail

Thanks for your interest in contributing! 🛡️

## How to Contribute

1. **Fork** the repository
2. **Clone** your fork locally
3. **Create a branch** for your feature or fix (`git checkout -b feature/my-feature`)
4. **Make your changes** and add tests if applicable
5. **Commit** with a clear message (`git commit -m "Add: description of change"`)
6. **Push** to your fork (`git push origin feature/my-feature`)
7. **Open a Pull Request** against `main`

## Development Setup

```bash
cp .env.example .env
docker-compose up -d postgres redis
cd backend && cargo build
cd ../frontend && npm install
