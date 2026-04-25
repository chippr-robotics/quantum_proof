import { useMemo, useState } from 'react';

// Anchor: measured Oath-32 numbers (n=32 field bits, w=8 window).
const ANCHOR_BITS = 32;
const ANCHOR_TOFFOLI = 5_760_000;
const ANCHOR_QUBITS = 2_848;

// Scaling exponents per published model.
const MODELS = {
  karatsuba: { label: 'Karatsuba', expGate: 2.585, expQubit: 1, color: '#16a34a' },
  schoolbook: { label: 'Schoolbook', expGate: 3, expQubit: 1, color: '#0ea5e9' },
  empirical: { label: 'Empirical (Oath fit)', expGate: 2.7, expQubit: 1.05, color: '#a855f7' },
} as const;

type ModelKey = keyof typeof MODELS;

function project(bits: number, model: ModelKey) {
  const ratio = bits / ANCHOR_BITS;
  const m = MODELS[model];
  const toffoli = Math.round(ANCHOR_TOFFOLI * Math.pow(ratio, m.expGate));
  const qubits = Math.round(ANCHOR_QUBITS * Math.pow(ratio, m.expQubit));
  return { toffoli, qubits };
}

function fmtCount(n: number) {
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return n.toLocaleString();
}

function fmtMins(toffoli: number) {
  // Reaction-limited: 10 µs per Toffoli (Google paper assumption).
  const seconds = toffoli * 10e-6;
  if (seconds < 60) return seconds.toFixed(0) + ' s';
  if (seconds < 3600) return (seconds / 60).toFixed(1) + ' min';
  return (seconds / 3600).toFixed(2) + ' h';
}

export default function ScalingCalculator() {
  const [bits, setBits] = useState(256);
  const [model, setModel] = useState<ModelKey>('karatsuba');

  const projection = useMemo(() => project(bits, model), [bits, model]);

  const presets = [
    { label: 'Oath-32 (measured)', bits: 32 },
    { label: 'Oath-64', bits: 64 },
    { label: 'secp256k1', bits: 256 },
    { label: 'P-384', bits: 384 },
    { label: 'P-521', bits: 521 },
  ];

  return (
    <div className="card reveal">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h3 className="text-lg font-bold text-slate-900 dark:text-white">Scaling projector</h3>
          <p className="text-sm text-slate-600 dark:text-slate-400">
            Anchored at the measured Oath-32 result (2,848 qubits, 5.76M Toffoli).
          </p>
        </div>
      </div>

      <div className="mt-6 grid gap-6 lg:grid-cols-2">
        <div className="space-y-5">
          <div>
            <label
              htmlFor="bits"
              className="flex items-center justify-between text-sm font-semibold text-slate-700 dark:text-slate-300"
            >
              Field width
              <span className="font-mono text-brand-700 dark:text-brand-300">{bits} bits</span>
            </label>
            <input
              id="bits"
              type="range"
              min={32}
              max={521}
              step={1}
              value={bits}
              onChange={(e) => setBits(Number(e.target.value))}
              className="mt-2 w-full accent-brand-600"
            />
            <div className="flex flex-wrap gap-1.5">
              {presets.map((p) => (
                <button
                  key={p.bits}
                  type="button"
                  onClick={() => setBits(p.bits)}
                  className={`rounded-full px-2.5 py-1 text-xs font-semibold transition-colors ${
                    bits === p.bits
                      ? 'bg-brand-600 text-white'
                      : 'bg-slate-100 text-slate-700 hover:bg-brand-100 hover:text-brand-800 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-brand-900/40'
                  }`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>

          <div>
            <p className="text-sm font-semibold text-slate-700 dark:text-slate-300">Scaling model</p>
            <div className="mt-2 flex flex-wrap gap-2">
              {(Object.keys(MODELS) as ModelKey[]).map((k) => (
                <button
                  key={k}
                  type="button"
                  onClick={() => setModel(k)}
                  className={`rounded-md border px-3 py-1.5 text-xs font-semibold transition-colors ${
                    model === k
                      ? 'border-brand-600 bg-brand-600 text-white'
                      : 'border-slate-300 bg-white text-slate-700 hover:border-brand-400 hover:text-brand-800 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300'
                  }`}
                >
                  {MODELS[k].label}
                </button>
              ))}
            </div>
            <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
              Toffoli ∝ n^{MODELS[model].expGate}, qubits ∝ n^{MODELS[model].expQubit}
            </p>
          </div>
        </div>

        <div className="space-y-3 rounded-xl bg-brand-50 p-5 dark:bg-brand-950/30">
          <div>
            <div className="text-xs font-semibold uppercase tracking-wider text-brand-700 dark:text-brand-300">
              Projected Toffoli
            </div>
            <div className="font-mono text-3xl font-extrabold text-brand-800 dark:text-brand-200">
              {fmtCount(projection.toffoli)}
            </div>
          </div>
          <div>
            <div className="text-xs font-semibold uppercase tracking-wider text-brand-700 dark:text-brand-300">
              Projected logical qubits
            </div>
            <div className="font-mono text-3xl font-extrabold text-brand-800 dark:text-brand-200">
              {fmtCount(projection.qubits)}
            </div>
          </div>
          <div>
            <div className="text-xs font-semibold uppercase tracking-wider text-brand-700 dark:text-brand-300">
              Reaction-limited runtime (10 µs/Toffoli)
            </div>
            <div className="font-mono text-2xl font-extrabold text-brand-800 dark:text-brand-200">
              {fmtMins(projection.toffoli)}
            </div>
          </div>
          <p className="border-t border-brand-200 pt-3 text-xs italic text-slate-600 dark:border-brand-800 dark:text-slate-400">
            Logical-only. Physical qubit overhead from surface-code error correction is
            multiplicative on top — see the Google paper for ≤500K physical qubits at 10⁻³ error.
          </p>
        </div>
      </div>
    </div>
  );
}
