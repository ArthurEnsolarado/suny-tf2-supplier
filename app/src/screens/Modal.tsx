import { useEffect, useRef, type ReactNode } from "react";

/** Janela sobre a tela: Esc fecha, o foco fica dentro e volta ao card ao fechar. */
export function Modal({ children, onClose, labelledBy }: { children: ReactNode; onClose: () => void; labelledBy: string }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (document.querySelector(".lightbox")) return; // o zoom da imagem trata o próprio Esc
        onClose();
        return;
      }
      if (e.key !== "Tab" || !ref.current) return;
      const items = [...ref.current.querySelectorAll<HTMLElement>('button, [href], input, select, [tabindex]:not([tabindex="-1"])')].filter(
        (el) => !el.hasAttribute("disabled"),
      );
      const first = items[0];
      const last = items[items.length - 1];
      if (!first || !last) return;
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      previous?.focus();
    };
  }, [onClose]);

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby={labelledBy} tabIndex={-1} ref={ref}>
        {children}
      </div>
    </div>
  );
}
