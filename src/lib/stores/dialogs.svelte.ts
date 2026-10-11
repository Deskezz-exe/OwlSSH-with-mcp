// A small promise-based text input dialog (new folder, rename, ...).

export interface TextRequest {
  title: string;
  label: string;
  value: string;
  submit: string;
}

class DialogsState {
  text = $state<TextRequest | null>(null);
  private resolve: ((value: string | null) => void) | null = null;

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
