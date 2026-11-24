import { memo } from "react";

interface HighlightTextProps {
  text: string;
  indices: number[];
}

/**
 * Renders text with character indices highlighted directly from Rust `nucleo-matcher`
 * + `pinyin` index projections (e.g. typing "wx" highlights both chars of "微信").
 */
export const HighlightText = memo(function HighlightText({
  text,
  indices,
}: HighlightTextProps) {
  if (!indices.length) {
    return <>{text}</>;
  }

  const hitSet = new Set(indices);
  const chars = Array.from(text);

  return (
    <>
      {chars.map((char, idx) =>
        hitSet.has(idx) ? (
          <mark
            key={idx}
            className="bg-transparent font-semibold text-blue-600 dark:text-blue-400"
          >
            {char}
          </mark>
        ) : (
          <span key={idx}>{char}</span>
        ),
      )}
    </>
  );
});
