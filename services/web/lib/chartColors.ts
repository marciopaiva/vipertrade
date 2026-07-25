/**
 * Cores para SVG/Recharts, derivadas dos tokens do tema.
 *
 * Charts recebem cor por atributo (`fill`, `stroke`), não por classe Tailwind,
 * então não dá para usar `text-accent` e pronto. Antes disso cada gráfico
 * escrevia o próprio hex — foi assim que o app acumulou quatro verdes e três
 * vermelhos para os mesmos dois conceitos.
 *
 * `hsl(var(--token))` é resolvido pelo browser em tempo de pintura, então o
 * gráfico passa a acompanhar o tema como o resto da interface.
 */

/** Cor semântica: o que o número significa. */
export const chart = {
  /** Lucro, alta, saudável. */
  positive: 'hsl(var(--accent))',
  /** Prejuízo, baixa, falha. */
  negative: 'hsl(var(--destructive))',
  /** Atenção — nem bom nem crítico. */
  warning: 'hsl(var(--warn))',
  /** Séries neutras e destaques que não carregam juízo de valor. */
  neutral: 'hsl(var(--primary))',

  /** Eixos e rótulos. */
  axis: 'hsl(var(--muted-foreground))',
  /** Linhas de grade — abaixo do eixo na hierarquia. */
  grid: 'hsl(var(--border))',
  /** Linha de referência (o zero), entre a grade e o eixo. */
  reference: 'hsl(var(--muted-foreground) / 0.55)',
  /** Fundo do painel — contorno de marcadores sobre a área do gráfico. */
  surface: 'hsl(var(--card))',
} as const;

/**
 * Escala sequencial de 5 passos para leituras 0-100 (medidor Fear & Greed).
 * Definida em `globals.css` (`--scale-1..5`); as pontas e o meio são os próprios
 * tokens semânticos, então a escala pertence à mesma família do resto.
 */
export const scale5 = [
  'hsl(var(--scale-1))',
  'hsl(var(--scale-2))',
  'hsl(var(--scale-3))',
  'hsl(var(--scale-4))',
  'hsl(var(--scale-5))',
] as const;

/** Passo da escala para uma leitura 0-100. */
export function scaleStep(value: number): string {
  if (value < 25) return scale5[0];
  if (value < 45) return scale5[1];
  if (value < 55) return scale5[2];
  if (value < 75) return scale5[3];
  return scale5[4];
}
