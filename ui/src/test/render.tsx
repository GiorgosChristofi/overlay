import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import type { Backend } from "../lib/backend";
import { BackendProvider } from "../lib/BackendContext";

export function renderWith(backend: Backend, ui: ReactElement) {
  return render(<BackendProvider backend={backend}>{ui}</BackendProvider>);
}
