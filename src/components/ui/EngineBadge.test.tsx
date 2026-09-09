import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EngineBadge } from "./EngineBadge";

describe("EngineBadge", () => {
  it("maps known engine ids to their display label", () => {
    render(<EngineBadge engine="postgres" />);
    expect(screen.getByText("PG")).toBeInTheDocument();
  });

  it("falls back to an uppercased engine id when unknown", () => {
    render(<EngineBadge engine="clickhouse" />);
    expect(screen.getByText("CLICKHOUSE")).toBeInTheDocument();
  });
});
