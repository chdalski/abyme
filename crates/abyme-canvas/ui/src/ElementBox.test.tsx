import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { ElementContent } from "./ElementBox";

afterEach(cleanup);

const MARKUP = [
  "<img src=x onerror=alert(1)>",
  "<script>alert(1)</script>",
  "javascript:alert(1)",
];

describe("ElementContent", () => {
  it("shows the element's type, name, and summary", () => {
    render(
      <ElementContent
        element="payments"
        shown={{
          kind: "element",
          type_ref: "c4/container",
          name: "Payment Service",
          summary: "Payment processing",
        }}
      />,
    );

    expect(screen.getByText("c4/container")).toBeDefined();
    expect(screen.getByText("Payment Service")).toBeDefined();
    expect(screen.getByText("Payment processing")).toBeDefined();
  });

  it("notes an element whose file is missing", () => {
    render(<ElementContent element="orders" shown={{ kind: "missing" }} />);

    expect(screen.getByText("Missing element orders")).toBeDefined();
  });

  it.each(MARKUP)("shows %s in every field as text", (text) => {
    const { container } = render(
      <ElementContent
        element="payments"
        shown={{ kind: "element", type_ref: text, name: text, summary: text }}
      />,
    );

    expect(screen.getAllByText(text)).toHaveLength(3);
    expect(container.querySelector("img, script, a[href]")).toBeNull();
  });
});
