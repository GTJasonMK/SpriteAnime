export function getById<T extends HTMLElement = HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`Missing DOM element: #${id}`);
  }
  return element as T;
}

export function queryOne<T extends Element = Element>(
  selector: string,
  root: ParentNode = document
): T {
  const element = root.querySelector<T>(selector);
  if (!element) {
    throw new Error(`Missing DOM element: ${selector}`);
  }
  return element;
}

export function queryOptional<T extends Element = Element>(
  selector: string,
  root: ParentNode = document
): T | null {
  return root.querySelector<T>(selector);
}

export function queryAll<T extends Element = Element>(
  selector: string,
  root: ParentNode = document
): T[] {
  return Array.from(root.querySelectorAll<T>(selector));
}
