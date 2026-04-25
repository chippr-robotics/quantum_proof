import { useEffect, useRef, useState } from 'react';

interface Props {
  chart: string;
  caption?: string;
}

export default function PipelineDiagram({ chart, caption }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const isDark = () => document.documentElement.classList.contains('dark');

    async function render() {
      if (!ref.current) return;
      try {
        const mermaid = (await import('mermaid')).default;
        mermaid.initialize({
          startOnLoad: false,
          theme: isDark() ? 'dark' : 'default',
          themeVariables: {
            primaryColor: '#dcfce7',
            primaryBorderColor: '#16a34a',
            primaryTextColor: isDark() ? '#dcfce7' : '#14532d',
            lineColor: isDark() ? '#86efac' : '#15803d',
            fontFamily: 'Inter, sans-serif',
            fontSize: '14px',
          },
          securityLevel: 'strict',
          flowchart: { curve: 'basis', padding: 12 },
        });
        const id = `mermaid-${Math.random().toString(36).slice(2, 9)}`;
        const { svg } = await mermaid.render(id, chart);
        if (!cancelled && ref.current) ref.current.innerHTML = svg;
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    }

    render();

    // Re-render on theme toggle.
    const observer = new MutationObserver(() => {
      if (!cancelled) render();
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });

    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [chart]);

  if (error) {
    return (
      <div className="rounded-lg border border-rose-300 bg-rose-50 p-4 text-sm text-rose-800 dark:border-rose-700 dark:bg-rose-950/40 dark:text-rose-200">
        Diagram failed to render: {error}
      </div>
    );
  }

  return (
    <figure className="my-8">
      <div
        ref={ref}
        className="flex w-full justify-center overflow-x-auto rounded-xl border border-slate-200 bg-white p-6 dark:border-slate-800 dark:bg-slate-900"
      />
      {caption && (
        <figcaption className="mt-2 text-center text-sm italic text-slate-500 dark:text-slate-400">
          {caption}
        </figcaption>
      )}
    </figure>
  );
}
