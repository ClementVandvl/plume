import { describe, expect, it } from "vitest";
import { chunksOf } from "./SplitPanel";

describe("chunksOf", () => {
  it("cuts on blank lines and rejoins to the original", () => {
    const latex = "Soient $u$ et $v$.\n\n\\begin{center}\nfigure\n\\end{center}\n\nAlors :";
    const chunks = chunksOf(latex);
    expect(chunks).toHaveLength(3);
    expect(chunks[1]).toBe("\\begin{center}\nfigure\n\\end{center}");
    // Any cut point must reconstruct the passage between the two halves.
    for (let at = 1; at < chunks.length; at += 1) {
      const head = chunks.slice(0, at).join("\n\n");
      const tail = chunks.slice(at).join("\n\n");
      expect(`${head}\n\n${tail}`).toBe(chunks.join("\n\n"));
    }
  });

  it("tolerates the whitespace a blank line really has", () => {
    expect(chunksOf("un\n   \ndeux\n\n\ntrois")).toEqual(["un", "deux", "trois"]);
    expect(chunksOf("\n\nseul\n\n")).toEqual(["seul"]);
  });

  it("reports a passage with nowhere to cut", () => {
    expect(chunksOf("une seule ligne")).toHaveLength(1);
    expect(chunksOf("   ")).toHaveLength(0);
  });
});
