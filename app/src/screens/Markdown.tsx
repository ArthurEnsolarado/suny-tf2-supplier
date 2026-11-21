import { Fragment, type ReactNode } from "react";

/** `**negrito**`, `` `código` `` e `[texto](link)` (só o texto: o app nunca segue link vindo de uma release). */
function inline(text: string, base: string): ReactNode[] {
  const clean = text.replace(/<[^>]*>/g, "");
  const out: ReactNode[] = [];
  const re = /(\*\*[^*]+\*\*|__[^_]+__|`[^`]+`|\[[^\]]+\]\([^)]*\))/g;
  let last = 0;
  let i = 0;
  for (const m of clean.matchAll(re)) {
    const at = m.index ?? 0;
    if (at > last) out.push(clean.slice(last, at));
    const tok = m[0];
    if (tok.startsWith("**") || tok.startsWith("__")) out.push(<b key={`${base}${i++}`}>{tok.slice(2, -2)}</b>);
    else if (tok.startsWith("`")) out.push(<code key={`${base}${i++}`}>{tok.slice(1, -1)}</code>);
    else out.push(tok.slice(1, tok.indexOf("]")));
    last = at + tok.length;
  }
  if (last < clean.length) out.push(clean.slice(last));
  return out;
}

/** Markdown simples (títulos, listas, parágrafos) como elementos do React. Nada de HTML solto. */
export function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  let list: string[] = [];
  let para: string[] = [];
  const flushList = () => {
    if (!list.length) return;
    const items = list;
    blocks.push(
      <ul key={`u${blocks.length}`}>
        {items.map((l, i) => (
          <li key={i}>{inline(l, `l${i}`)}</li>
        ))}
      </ul>,
    );
    list = [];
  };
  const flushPara = () => {
    if (!para.length) return;
    const lines = para;
    blocks.push(
      <p key={`p${blocks.length}`}>
        {lines.map((l, i) => (
          <Fragment key={i}>
            {i > 0 && <br />}
            {inline(l, `p${i}`)}
          </Fragment>
        ))}
      </p>,
    );
    para = [];
  };
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trimEnd();
    const h = /^#{1,6}\s+(.*)$/.exec(line);
    const li = /^\s*[-*+]\s+(.*)$/.exec(line);
    if (h) {
      flushList();
      flushPara();
      blocks.push(<h4 key={`h${blocks.length}`}>{inline(h[1] ?? "", "h")}</h4>);
    } else if (li) {
      flushPara();
      list.push(li[1] ?? "");
    } else if (!line.trim()) {
      flushList();
      flushPara();
    } else {
      flushList();
      para.push(line);
    }
  }
  flushList();
  flushPara();
  return <div className="md">{blocks}</div>;
}
