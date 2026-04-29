import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import { tick } from "svelte";
import Page from "./+page.svelte";

// Cast mock for type-safe usage
const mockInvoke = vi.mocked(invoke);

/** Set input value and trigger Svelte's bind:value update. */
async function typeInInput(el: HTMLInputElement, value: string) {
  // Set the DOM property, then fire `input` via testing-library (wraps in act/flushSync)
  el.value = value;
  await fireEvent.input(el);
  await tick();
}

/** Wait for the component's onMount to finish loading commands. */
async function waitForReady() {
  await vi.waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("list_commands");
  });
  // Let Svelte flush the state updates from the resolved promise
  await tick();
  await new Promise(r => setTimeout(r, 0));
  await tick();
}

beforeEach(() => {
  vi.clearAllMocks();

  // Default mock responses for the onMount sequence
  mockInvoke.mockImplementation(async (cmd: string, _args?: unknown) => {
    switch (cmd) {
      case "get_settings":
        return {
          hotkey: "Super+Space",
          show_context_chip: true,
          allow_duplicates: true,
          shared_dir: "shared",
          seed_examples: false,
        };
      case "load_context":
        return "";
      case "is_debug":
        return false;
      case "list_commands":
        return {
          commands: [
            {
              phrase: "open google",
              title: "Search Google",
              env: {},
              source_dir: "",
              source_file: "open-google.yaml",
              action: { type: "open_url", config: { url: "https://google.com" } },
            },
            {
              phrase: "paste greeting",
              title: "Paste a greeting",
              env: {},
              source_dir: "",
              source_file: "paste-greeting.yaml",
              action: { type: "paste_text", config: { text: "Hello!" } },
            },
          ],
          duplicates: [],
          reserved: [],
          skipped: [],
          warnings: [],
        };
      case "register_shortcut":
        return undefined;
      case "save_context":
        return undefined;
      case "hide_window":
        return undefined;
      default:
        return undefined;
    }
  });
});

describe("Launcher page", () => {
  it("renders the input field with placeholder", async () => {
    render(Page);
    // Wait for onMount to settle
    await vi.waitFor(() => {
      const input = screen.getByPlaceholderText("Type a command…");
      expect(input).toBeInTheDocument();
    });
  });

  it("renders the prompt glyph", async () => {
    render(Page);
    await vi.waitFor(() => {
      expect(screen.getByText("»")).toBeInTheDocument();
    });
  });

  it("shows 'No matching commands' when typing a non-matching query", async () => {
    render(Page);
    await vi.waitFor(() => {
      expect(screen.getByPlaceholderText("Type a command…")).toBeInTheDocument();
    });

    const input = screen.getByPlaceholderText("Type a command…") as HTMLInputElement;
    await typeInInput(input, "zzzznotacommand");

    await vi.waitFor(() => {
      expect(screen.getByText("No matching commands")).toBeInTheDocument();
    });
  });

  it("shows matching commands when typing a partial phrase", async () => {
    render(Page);
    await waitForReady();

    const input = screen.getByPlaceholderText("Type a command…") as HTMLInputElement;
    await typeInInput(input, "open");

    await vi.waitFor(() => {
      expect(screen.getByText("Search Google")).toBeInTheDocument();
    });
  });

  it("shows action badges on results", async () => {
    render(Page);
    await waitForReady();

    const input = screen.getByPlaceholderText("Type a command…") as HTMLInputElement;
    await typeInInput(input, "open");

    await vi.waitFor(() => {
      expect(screen.getByText("URL")).toBeInTheDocument();
    });
  });

  it("filters to only matching commands", async () => {
    render(Page);
    await waitForReady();

    const input = screen.getByPlaceholderText("Type a command…") as HTMLInputElement;
    await typeInInput(input, "greeting");

    await vi.waitFor(() => {
      expect(screen.getByText("Paste a greeting")).toBeInTheDocument();
      expect(screen.queryByText("Search Google")).not.toBeInTheDocument();
    });
  });

  it("does not show results when input is empty", async () => {
    render(Page);
    await vi.waitFor(() => {
      expect(screen.getByPlaceholderText("Type a command…")).toBeInTheDocument();
    });

    expect(screen.queryByText("No matching commands")).not.toBeInTheDocument();
    expect(screen.queryByText("Search Google")).not.toBeInTheDocument();
  });

  it("shows builtin commands when typing /", async () => {
    render(Page);
    await vi.waitFor(() => {
      expect(screen.getByPlaceholderText("Type a command…")).toBeInTheDocument();
    });

    const input = screen.getByPlaceholderText("Type a command…") as HTMLInputElement;
    await typeInInput(input, "/ctx");

    await vi.waitFor(() => {
      expect(screen.getByText("Set context")).toBeInTheDocument();
      expect(screen.getByText("Reset context")).toBeInTheDocument();
    });
  });

  it("shows duplicate warnings when present", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "get_settings":
          return { hotkey: "Super+Space", show_context_chip: true, allow_duplicates: true, shared_dir: "shared", seed_examples: false };
        case "load_context":
          return "";
        case "is_debug":
          return false;
        case "list_commands":
          return {
            commands: [],
            duplicates: [{ phrase: "open google", kept: "a.yaml", ignored: "b.yaml" }],
            reserved: [],
            skipped: [],
            warnings: [],
          };
        case "register_shortcut":
          return undefined;
        case "save_context":
          return undefined;
        default:
          return undefined;
      }
    });

    render(Page);

    await vi.waitFor(() => {
      expect(screen.getByText(/1 command ignored/)).toBeInTheDocument();
    });
  });
});
