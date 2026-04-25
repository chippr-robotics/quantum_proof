import { useState } from 'react';

export default function AttackCalculator() {
  const [attack, setAttack] = useState(9);
  const [block, setBlock] = useState(10);

  const probability = Math.exp(-attack / block);
  const pct = (probability * 100).toFixed(1);

  return (
    <div className="card reveal max-w-xl">
      <h3 className="text-lg font-bold text-slate-900 dark:text-white">Attack success calculator</h3>
      <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
        Block confirmation times follow an exponential distribution. Estimate the probability that
        a quantum attacker recovers a key before the next block confirms.
      </p>

      <div className="mt-6 space-y-5">
        <div>
          <label htmlFor="attack" className="flex items-center justify-between text-sm font-semibold text-slate-700 dark:text-slate-300">
            Quantum attack time
            <span className="font-mono text-brand-700 dark:text-brand-300">{attack} min</span>
          </label>
          <input
            id="attack"
            type="range"
            min={1}
            max={60}
            value={attack}
            onChange={(e) => setAttack(Number(e.target.value))}
            className="mt-2 w-full accent-brand-600"
          />
          <div className="mt-1 flex justify-between text-xs text-slate-500 dark:text-slate-400">
            <span>1 min</span>
            <span>60 min</span>
          </div>
        </div>

        <div>
          <label htmlFor="block" className="flex items-center justify-between text-sm font-semibold text-slate-700 dark:text-slate-300">
            Average block time
            <span className="font-mono text-brand-700 dark:text-brand-300">{block} min</span>
          </label>
          <input
            id="block"
            type="range"
            min={0.5}
            max={30}
            step={0.5}
            value={block}
            onChange={(e) => setBlock(Number(e.target.value))}
            className="mt-2 w-full accent-brand-600"
          />
          <div className="mt-1 flex justify-between text-xs text-slate-500 dark:text-slate-400">
            <span>0.5 min</span>
            <span>30 min</span>
          </div>
        </div>

        <div className="rounded-xl bg-brand-50 p-5 text-center dark:bg-brand-950/40">
          <div className="font-mono text-4xl font-extrabold text-brand-800 dark:text-brand-200">{pct}%</div>
          <p className="mt-2 text-xs text-slate-600 dark:text-slate-400">
            Probability the block takes longer than the attack
          </p>
          <p className="mt-1 font-mono text-xs text-slate-500 dark:text-slate-500">
            P = e<sup>−{attack}/{block}</sup> = {probability.toFixed(4)}
          </p>
        </div>
      </div>
    </div>
  );
}
