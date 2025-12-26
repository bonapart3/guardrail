# GuardRail Deployment Guide

This guide covers deploying GuardRail to production environments. The system consists of 5 Rust microservices, a Next.js frontend, and smart contracts on Ethereum and Solana.

## Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐
│   Vercel        │    │     Fly.io      │
│   Frontend      │◄──►│   API Gateway   │
│   (Next.js)     │    │     (Rust)      │
└─────────────────┘    └─────────────────┘
                              │
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
            ┌────────────┐ ┌─────┐ ┌─────────┐
            │ Identity   │ │Policy│ │Movement │
            │ Service    │ │Engine│ │ Ledger  │
            └────────────┘ └─────┘ └─────────┘
                    ▼
            ┌────────────┐
            │ Chain      │
            │ Anchor     │
            └────────────┘
                    │
            ┌───────┼───────┐
            ▼       ▼       ▼
        ┌─────┐ ┌─────┐ ┌─────┐
        │ Eth │ │Supa │ │ Sol │
        │ L2  │ │Base │ │     │
        └─────┘ └─────┘ └─────┘
```

## Prerequisites

- [Fly.io CLI](https://fly.io/docs/flyctl/install/)
- [Vercel CLI](https://vercel.com/docs/cli)
- [Supabase account](https://supabase.com/)
- [Foundry](https://book.getfoundry.sh/) (for Ethereum contracts)
- [Anchor](https://www.anchor-lang.com/) (for Solana contracts)
- Domain names configured

## Environment Variables

### Backend Services

Create a `.env` file for each service with the following variables:

```bash
# Database
DATABASE_URL=postgresql://user:pass@host:5432/db

# Redis
REDIS_URL=redis://host:6379

# JWT
JWT_SECRET=your-256-bit-secret

# API Keys
ETHERSCAN_API_KEY=your-etherscan-key
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

# Blockchain
ETHEREUM_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
ETHEREUM_PRIVATE_KEY=your-deployer-private-key

# Logging
RUST_LOG=info
LOG_FORMAT=json
```

### Frontend

```bash
NEXT_PUBLIC_API_URL=https://api.guardrail.dev
NEXT_PUBLIC_ENVIRONMENT=production
```

## Database Setup (Supabase)

1. Create a new Supabase project
2. Run the database migrations:

```bash
# Connect to Supabase
psql "postgresql://postgres:[password]@[host]:5432/postgres"

# Run the schema
\i scripts/init.sql
```

3. Set up Row Level Security policies for multi-tenancy
4. Configure database backups

## Backend Deployment (Fly.io)

### Service Configuration

Each service needs a `fly.toml` configuration file:

```toml
app = "guardrail-api-gateway"
primary_region = "iad"

[build]
  dockerfile = "Dockerfile"

[env]
  PORT = "8080"

[http_service]
  internal_port = 8080
  force_https = true
  auto_stop_machines = true
  auto_start_machines = true
  min_machines_running = 1
  processes = ["app"]

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory_mb = 512
```

### Deployment Steps

1. **Install Fly CLI and login:**
```bash
fly auth login
```

2. **Deploy each service:**
```bash
# API Gateway
cd backend/api-gateway
fly launch --no-deploy
fly deploy

# Identity Service
cd ../identity-service
fly launch --no-deploy
fly deploy

# Policy Engine
cd ../policy-engine
fly launch --no-deploy
fly deploy

# Movement Ledger
cd ../movement-ledger
fly launch --no-deploy
fly deploy

# Chain Anchor
cd ../chain-anchor
fly launch --no-deploy
fly deploy
```

3. **Configure secrets:**
```bash
fly secrets set DATABASE_URL="postgresql://..."
fly secrets set JWT_SECRET="..."
fly secrets set REDIS_URL="redis://..."
```

4. **Set up custom domains:**
```bash
fly certs add api.guardrail.dev
```

5. **Configure health checks:**
Each service exposes `/health` endpoint for load balancer health checks.

## Frontend Deployment (Vercel)

1. **Install Vercel CLI:**
```bash
npm i -g vercel
vercel login
```

2. **Deploy frontend:**
```bash
cd frontend
vercel --prod
```

3. **Configure environment variables:**
```bash
vercel env add NEXT_PUBLIC_API_URL
vercel env add NEXT_PUBLIC_ENVIRONMENT
```

4. **Set up custom domain:**
```bash
vercel domains add guardrail.dev
```

## Smart Contract Deployment

### Ethereum (Foundry)

1. **Install dependencies:**
```bash
cd contracts/ethereum
forge install
```

2. **Configure environment:**
```bash
cp .env.example .env
# Edit .env with your RPC URL and private key
```

3. **Deploy contract:**
```bash
forge script script/Deploy.s.sol --rpc-url $ETHEREUM_RPC_URL --private-key $PRIVATE_KEY --broadcast --verify
```

4. **Verify deployment:**
```bash
forge verify-contract --chain-id 1 --etherscan-api-key $ETHERSCAN_API_KEY <contract-address> src/GuardRailAnchor.sol:GuardRailAnchor
```

### Solana (Anchor)

1. **Install dependencies:**
```bash
cd contracts/solana
anchor build
```

2. **Configure wallet:**
```bash
solana config set --url mainnet-beta
solana config set --keypair ~/.config/solana/id.json
```

3. **Deploy program:**
```bash
anchor deploy
```

4. **Initialize program:**
```bash
anchor run initialize
```

## Staging Environment

### Setup Steps

1. **Create staging apps on Fly.io:**
```bash
fly launch --name guardrail-api-gateway-staging
fly launch --name guardrail-identity-staging
# ... repeat for each service
```

2. **Deploy to staging:**
```bash
fly deploy --app guardrail-api-gateway-staging
```

3. **Set up staging database:**
Use a separate Supabase project or database schema for staging.

4. **Configure staging domains:**
- `staging-api.guardrail.dev`
- `staging.guardrail.dev`

### Staging Checklist

- [ ] All services deploy successfully
- [ ] Database migrations run
- [ ] Environment variables configured
- [ ] Health checks pass
- [ ] Frontend connects to staging API
- [ ] Basic functionality tested
- [ ] Contracts deployed to testnets

## Monitoring & Observability

### Application Monitoring

1. **Health Checks:**
All services expose `/health` endpoints that check:
- Database connectivity
- Redis connectivity
- Service dependencies

2. **Metrics:**
- Request latency
- Error rates
- Database query performance
- Blockchain transaction status

3. **Logging:**
- Structured JSON logs
- Centralized log aggregation (recommended: Axiom, Datadog)
- Error alerting

### Infrastructure Monitoring

1. **Fly.io Metrics:**
- CPU/Memory usage
- Request throughput
- Error rates per service

2. **Database Monitoring:**
- Connection pool usage
- Query performance
- Backup status

3. **Blockchain Monitoring:**
- Transaction confirmation times
- Gas costs
- Failed transaction alerts

## Security Considerations

### API Security

- JWT tokens with short expiration
- API key authentication for service-to-service
- Rate limiting on public endpoints
- CORS configuration
- Input validation and sanitization

### Infrastructure Security

- Private networking between services
- Encrypted database connections
- Secure secret management
- Regular dependency updates
- Container vulnerability scanning

### Compliance

- Data encryption at rest
- Audit logging for all actions
- GDPR compliance for EU users
- SOC 2 compliance preparation

## Rollback Procedures

### Service Rollback

```bash
# Check deployment history
fly releases --app guardrail-api-gateway

# Rollback to previous version
fly releases rollback <version-id>
```

### Database Rollback

1. Restore from backup
2. Run migration rollback scripts
3. Verify data integrity

### Contract Rollback

1. Deploy new contract version
2. Update service configurations
3. Migrate existing data if needed

## Performance Optimization

### Backend Services

- Connection pooling for database
- Redis caching for frequently accessed data
- Async processing for blockchain operations
- Horizontal scaling with multiple instances

### Frontend

- Static asset optimization
- API response caching
- Code splitting
- CDN configuration

### Database

- Query optimization
- Index maintenance
- Connection pooling
- Read replicas for analytics

## Troubleshooting

### Common Issues

1. **Service startup failures:**
   - Check environment variables
   - Verify database connectivity
   - Check service logs: `fly logs --app <app-name>`

2. **Database connection issues:**
   - Verify connection string
   - Check firewall rules
   - Monitor connection pool usage

3. **Blockchain transaction failures:**
   - Check RPC endpoint status
   - Verify gas prices
   - Monitor transaction queues

### Debug Commands

```bash
# Check service status
fly status --app <app-name>

# View logs
fly logs --app <app-name>

# SSH into running instance
fly ssh console --app <app-name>

# Check environment
fly secrets list --app <app-name>
```

## Maintenance

### Regular Tasks

- **Weekly:**
  - Review error logs
  - Check disk usage
  - Update dependencies

- **Monthly:**
  - Security updates
  - Performance reviews
  - Backup verification

- **Quarterly:**
  - Load testing
  - Disaster recovery testing
  - Compliance audits

### Backup Strategy

- Database: Daily automated backups via Supabase
- Application: Infrastructure as code in Git
- Contracts: Source code versioning
- Keys: Secure key management system

## Support

For deployment issues:
- Check service logs
- Review health check endpoints
- Contact DevOps team
- Check GitHub issues

## Version History

- v1.0.0: Initial production deployment
- v1.1.0: Added rate limiting and monitoring
- v1.2.0: Multi-region deployment support