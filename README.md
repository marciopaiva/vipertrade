# 🐍 VIPERTRADE v0.8.1

> **Lead Trader Bot para Bybit Copy Trading Classic**
> *Otimizado para Smart Copy Mode com Trailing Stop Dinâmico*

## 📋 Visão Geral

ViperTrade é um sistema de trading automatizado projetado para operar como **Lead Trader** na Bybit. Ele utiliza uma arquitetura de microsserviços orientada a eventos e uma engine de estratégia determinística baseada na linguagem **Tupã**.

### Principais Diferenciais
- **Tupã Engine**: Lógica de trading auditável e determinística.
- **Smart Copy Optimized**: Otimização de position sizing para maximizar a taxa de sucesso dos seguidores.
- **Dynamic Trailing Stop**: Proteção de lucros com mecanismo de "ratcheting".
- **Risk Management Multi-Camada**: Circuit breakers e limites de perda diária.

## 🚀 Como Iniciar

### Pré-requisitos
- Podman & Podman Compose
- Rust (para desenvolvimento)
- Conta na Bybit (Testnet para desenvolvimento)

### Setup Inicial

1. **Clone o repositório**
   ```bash
   git clone https://github.com/seu-usuario/vipertrade.git
   cd vipertrade
   ```

2. **Inicialize os Segredos**
   ```bash
   ./scripts/init-secrets.sh
   ```

3. **Verifique a Segurança**
   ```bash
   ./scripts/security-check.sh
   ```

4. **Inicie os Serviços**
   ```bash
   cd compose
   podman-compose up -d
   ```

## 🏗 Arquitetura

O sistema é composto pelos seguintes serviços:

- **Market Data**: Ingestão de dados via WebSocket da Bybit.
- **Strategy**: Engine Tupã + Gerenciamento de Risco.
- **Executor**: Execução de ordens via REST API.
- **Monitor**: Health checks, reconciliação e notificações Discord.
- **Database**: PostgreSQL para persistência de trades e eventos.
- **Cache**: Redis para comunicação Pub/Sub entre serviços.

## 📚 Documentação

Para detalhes completos da especificação técnica, consulte [VIPERTRADE_SPEC.md](VIPERTRADE_SPEC.md).

---
**Status:** Em desenvolvimento (Bloco 1: Setup)
