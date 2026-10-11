// A small promise-based text input dialog (new folder, rename, ...).

export interface TextRequest {
  title: string;
  label: string;
  value: string;
  submit: string;
}

export interface ConflictRequest {
  name: string;
  isDir: boolean;
  /** Where the name is taken. */
  side: "remote" | "local";
  /** A free name to suggest for renaming. */
  suggestion: string;
  /** How many more conflicts come after this one. */
  remaining: number;
  /** True when a name is already used at the destination. */
  isTaken: (name: string) => Promise<boolean>;
}

export type ConflictAnswer =
  | { action: "replace"; all: boolean }
  | { action: "rename"; name: string; all: boolean }
  | { action: "cancel"; all: boolean };

class DialogsState {
  text = $state<TextRequest | null>(null);
  conflict = $state<ConflictRequest | null>(null);
  private resolve: ((value: string | null) => void) | null = null;
  private resolveConflict: ((value: ConflictAnswer) => void) | null = null;

  askConflict(request: ConflictRequest): Promise<ConflictAnswer> {
    this.resolveConflict?.({ action: "cancel", all: true });
    this.conflict = request;
    return new Promise((resolve) => (this.resolveConflict = resolve));
  }

  answerConflict(answer: ConflictAnswer) {
    const resolve = this.resolveConflict;
    this.resolveConflict = null;
    this.conflict = null;
    resolve?.(answer);
  }

  askText(request: TextRequest): Promise<string | null> {
    this.resolve?.(null);
    this.text = request;
    return new Promise((resolve) => (this.resolve = resolve));
  }

  answer(value: string | null) {
    const resolve = this.resolve;
    this.resolve = null;
    this.text = null;
    resolve?.(value);
  }
}

export const dialogs = new DialogsState();
