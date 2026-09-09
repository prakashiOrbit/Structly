import { useEffect, useRef } from "react";
import * as echarts from "echarts";
import type { EChartsOption } from "echarts";

export function EChart({ option, className }: { option: EChartsOption; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!ref.current) return;
    const chart = echarts.init(ref.current);
    chart.setOption(option);

    const resize = () => chart.resize();
    const observer = new ResizeObserver(resize);
    observer.observe(ref.current);

    return () => {
      observer.disconnect();
      chart.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(option)]);

  return <div ref={ref} className={className} />;
}
