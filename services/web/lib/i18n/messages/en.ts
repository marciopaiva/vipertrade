// English catalog. This file is the TYPE SOURCE of truth for messages — pt-BR.ts
// must satisfy the same shape. Flat namespaces; values may contain {var} placeholders
// interpolated by useT(). Keep keys stable; add a namespace as a screen is migrated.
const en = {
  nav: {
    console: 'Console',
    strategy: 'Strategy',
    trades: 'Trades',
    analysis: 'Analysis',
    system: 'System',
  },
  common: {
    language: 'Language',
    density: 'Density',
    logout: 'Log out',
    loading: 'Loading…',
    error: 'Error',
    generate: 'Generate',
    regenerate: 'Regenerate',
    running: 'Running…',
    copy: 'copy',
    copied: 'copied ✓',
  },
  analysis: {
    title: 'Analysis',
    subtitle:
      'Live operation quality (realized trades) and a deterministic tuning simulation (what-if). Config changes are applied manually.',
    tabLive: 'Live',
    tabWhatif: 'What-if (grid)',
  },
  live: {
    blurb:
      'Operation quality over realized (closed, paper) trades — real data, not a backtest. Refreshes every 30s.',
    kpiNet: 'Realized net PnL',
    kpiWin: 'Win rate',
    kpiClosed: 'Closed trades',
    kpiCapture: 'Peak capture',
    empty: 'No closed trades in the last {days}d.',
    followTitle: 'Entry follow-through · armed the trail vs died flat',
    followArmed: 'Armed the trail (moved ≥ +0.1%)',
    followNotArmed: 'Never armed (entered and went nowhere)',
    peakTitle: 'Peak capture · trailing exits ({n})',
    peakAvg: 'avg peak',
    peakRealized: 'realized',
    peakLocked: 'locked',
    reasonsTitle: 'Attribution by exit reason',
    bySymbolTitle: 'By token · worst first',
    colSymbol: 'Symbol',
    colNet: 'net PnL',
    colTrades: 'Trades',
    colWin: 'Win%',
    statTrades: '{n} trades',
    statWins: '{n} wins',
    statAvg: 'avg',
  },
  whatif: {
    blurb:
      'Deterministic backtest grid (paths/PnL computed in Rust) over the audit corpus. On-demand. ⚠️ The backtest does NOT model live trailing (advice/min_hold) — trust the entry axes; validate trailing on the Live tab.',
    kpiCorpus: 'Corpus ticks',
    kpiBaseline: 'Baseline net PnL',
    kpiWin: 'Win rate',
    kpiClosed: 'Closed',
    recTitle: 'Recommendation · apply manually (no auto-apply)',
    recBest:
      'Best alpha variant with a positive delta on the corpus. Exposure variants (size-only) are never recommended as tuning.',
    recNone:
      'No alpha improvement on the current corpus — keep the config. (Exposure variants do not count as a strategy improvement.)',
    gridTitle: 'Deterministic grid · sorted by Δ net PnL (explicit sign)',
    colAxis: 'Axis',
    colValue: 'Value',
    colClass: 'Class',
    colDelta: 'Δ net PnL',
    colNet: 'net PnL',
    colWL: 'W/L',
    subTitle: 'Token substitution · hypothesis (candidates have no corpus, not backtested)',
    subWorst: 'Worst symbol: {symbol}',
    subPool: 'Substitution pool (disabled, validate in paper first):',
    subNone: 'No drop candidate with enough trades.',
    empty: 'Click “Generate” to run the tuning grid.',
  },
  positions: {
    title: 'Open Positions',
    titleCount: 'Open Positions ({n})',
    flatNoPos: 'Flat — no open positions',
    flatGuards: 'Flat — guards holding {n} setups',
    flatNote:
      'The strategy is monitoring the market; entries open when exchange consensus and the entry guards align.',
    stop: 'stop',
    peak: 'peak',
    trail: 'trail',
    trailArmed: '🔒 trail armed',
    trailLocked: '🔒 locks {pct}',
    tpAt: 'TP at {pct}',
    tpBeyond: 'TP ✓ {pct} beyond',
    tpMax: ' · max {pct}',
    cushion: 'cushion to stop',
    entry: 'entry',
    mark: 'mark',
    tpArm: 'TP-arm',
  },
  console: {
    connecting: 'Connecting to ViperTrade',
    // KPI strip
    equity: 'Equity',
    delta24h: '24h',
    winRate: 'Win rate',
    open: 'Open',
    today: 'Today',
    trades: 'trades',
    // Market sentiment
    sentimentTitle: 'Market Sentiment',
    fngTitle: 'Fear & Greed Index',
    indexUnavailable: 'Index unavailable',
    fngBlurb:
      'The Crypto Fear & Greed Index distills volatility, momentum, volume, social media, and surveys into a single 0–100 reading of market emotion.',
    fngExtremeFear: 'Extreme Fear',
    fngExtremeFearNote:
      'Investors are highly fearful — often an oversold zone with potential buying opportunities.',
    fngFear: 'Fear',
    fngFearNote: 'Caution dominates the market — sentiment leans bearish.',
    fngNeutral: 'Neutral',
    fngNeutralNote: 'Sentiment is balanced — no strong directional bias.',
    fngGreed: 'Greed',
    fngGreedNote: 'Optimism is rising — momentum is bullish, but watch for froth.',
    fngExtremeGreed: 'Extreme Greed',
    fngExtremeGreedNote:
      'Markets are euphoric — historically a zone of elevated pullback risk.',
    longShortRatio: 'Long/Short ratio',
    bybitPerps: 'Bybit perpetuals',
    long: 'Long',
    short: 'Short',
    longShortUnavailable: 'Long/Short ratio unavailable',
    // Equity curve
    equityTitle: 'Equity curve',
    equitySubtitle:
      'Cumulative realized PnL across {n} closed trades · marker per fill',
    equityNet: 'net',
    equityPeak: 'peak',
    equityEmpty: 'No closed trades yet — the curve plots as fills land.',
    fill: 'fill',
    equityLabel: 'equity',
  },
};

// Value types widen to `string` (no `as const`) so pt-BR.ts can hold translated
// strings while still being checked against the same key shape.
export type Messages = typeof en;
export default en;
