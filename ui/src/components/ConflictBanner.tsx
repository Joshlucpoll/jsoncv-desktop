type Props = { canShow: boolean; busy: boolean; onShow(): void; onDiscard(): void; onOverwrite(): void }

export function ConflictBanner({ canShow, busy, onShow, onDiscard, onOverwrite }: Props) {
  return (
    <div className="banner conflict" role="alert">
      <span>This CV changed while you were editing</span>
      <div className="actions">
        <button disabled={!canShow} onClick={onShow}>Show changes</button>
        <button disabled={busy} onClick={onDiscard}>Discard my edits</button>
        <button disabled={busy} onClick={onOverwrite}>Overwrite</button>
      </div>
    </div>
  )
}
