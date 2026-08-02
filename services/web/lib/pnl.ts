import type { Trade } from '@/types/trading';

/**
 * Resultado que de fato entrou na carteira: bruto menos taxa e funding.
 *
 * O campo `pnl` do backend é BRUTO. Com posição de ~$8 a taxa de ida e volta
 * (0,11%) chega a superar o ganho do trade, então somar `pnl` direto mostra
 * como vitória operações que perderam dinheiro — foi o que aconteceu com o
 * TIAUSDT de 02/08: +$0,0068 de bruto contra $0,0082 de taxa.
 */
export function netPnl(t: Pick<Trade, 'pnl' | 'fees' | 'funding_paid' | 'net_pnl'>) {
  if (typeof t.net_pnl === 'number') return t.net_pnl;
  if (typeof t.pnl !== 'number') return 0;
  return t.pnl - (t.fees ?? 0) - (t.funding_paid ?? 0);
}

/** Um trade só é vitória se sobrou dinheiro depois dos custos. */
export function isWin(t: Pick<Trade, 'pnl' | 'fees' | 'funding_paid' | 'net_pnl'>) {
  return netPnl(t) > 0;
}
