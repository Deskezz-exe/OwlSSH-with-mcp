export interface Toast {
  id: number;
  text: string;
  tone: "info" | "error";
}

let counter = 0;

class ToastState {
  list = $state<Toast[]>([]);

  show(text: string, tone: Toast["tone"] = "info", ms = 3500) {
    const id = ++counter;
    this.list.push({ id, text, tone });
    setTimeout(() => this.dismiss(id), ms);
  }

  dismiss(id: number) {
    this.list = this.list.filter((t) => t.id !== id);
  }
}

export const toast = new ToastState();
