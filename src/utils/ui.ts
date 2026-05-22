export function setButtonState(
  button: HTMLButtonElement,
  options: {
    disabled?: boolean;
    loading?: boolean;
    text?: string;
  }
): void {
  if (options.disabled !== undefined) {
    button.disabled = options.disabled;
  }
  if (options.loading !== undefined) {
    button.classList.toggle("is-loading", options.loading);
  }
  if (options.text !== undefined) {
    button.textContent = options.text;
  }
}

export function setPanelVisible(
  element: HTMLElement,
  visible: boolean,
  display: string = ""
): void {
  element.style.display = visible ? display : "none";
}

export function setBusyState(
  container: HTMLElement,
  textElement: HTMLElement,
  visible: boolean,
  text: string
): void {
  container.hidden = !visible;
  textElement.textContent = text;
}
