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

## Arquitetura

O sistema é composto por 8 serviços principais rodando em containers (Podman/Docker):

1. **Market Data Service**: Coleta dados de mercado via WebSocket (Bybit) e publica no Redis.
2. **Strategy Engine**: Processa dados de mercado usando pipelines Tupã (.tp) e gera sinais.
3. **Execution Service**: Recebe sinais e executa ordens na exchange.
4. **Risk Monitor**: Monitora exposição, PnL e saúde do sistema.
5. **Backtest Engine**: Executa simulações de estratégias com dados históricos.
6. **Web Interface**: Dashboard para visualização de métricas e controle manual (Next.js).
7. **API Service**: Backend REST para a interface web e integrações externas.
8. **Infrastructure**:
   - **PostgreSQL**: Persistência de trades, snapshots e logs de auditoria.
   - **Redis**: Barramento de mensagens e cache de alta velocidade.

Todos os serviços se comunicam através de uma rede isolada `vipertrade-net` (subnet 172.20.0.0/16).

## 📚 Documentação

Para detalhes completos da especificação técnica, consulte [VIPERTRADE_SPEC.md](VIPERTRADE_SPEC.md).

---
**Status:** Em desenvolvimento (Bloco 1: Setup)
