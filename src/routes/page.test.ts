import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { tick } from "svelte";
import Page from "./+page.svelte";

// Cast mocks for type-safe usage
const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

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

// Default mock responses for the onMount sequence
async function defaultMock(cmd: string, _args?: unknown): Promise<unknown> {
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
            {
              phrase: "pick customer",
              title: "Pick a customer",
              env: {},
              source_dir: "",
              source_file: "pick-customer.yaml",
              action: { type: "static_list", config: { list: "customers", item_action: "ctx_set" } },
            },
            {
              phrase: "pick vendor",
              title: "Pick a vendor",
              env: {},
              source_dir: "",
              source_file: "pick-vendor.yaml",
              action: { type: "static_list", config: { list: "vendors", item_action: "ctx_set" } },
            },
          ],
          duplicates: [],
          reserved: [],
          skipped: [],
          warnings: [],
        };
    case "load_list":
        if ((_args as { listName?: string })?.listName === "customers") {
          return [{ title: "Acme Corp", subtext: "acme-id" }];
        }
        if ((_args as { listName?: string })?.listName === "vendors") {
          return [{ title: "Globex" }];
        }
        return [];
    case "register_shortcut":
        return undefined;
    case "save_context":
        return undefined;
    case "hide_window":
        return undefined;
    default:
        return undefined;
    }
}

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation(defaultMock);
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

  it("matches using typed text only when a context is set", async () => {
    // Context is ambient-only: it must never be appended to the typed input
    // for matching. With context "zzz", typing "google" must still match the
    // "open google" command (the appended "google zzz" would match nothing).
    mockInvoke.mockImplementation(async (cmd: string, _args?: unknown) => {
      if (cmd === "load_context") return "zzz";
      return defaultMock(cmd);
    });

    render(Page);
    await waitForReady();

    // The context chip proves the context is active.
    await vi.waitFor(() => {
      expect(screen.getByText("zzz")).toBeInTheDocument();
    });

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "google");

    await vi.waitFor(() => {
      expect(screen.getByText("Search Google")).toBeInTheDocument();
    });
  });

  it("never passes the active context as a command param", async () => {
    // With context "reddit", executing "open google" must pass param: null —
    // the context must not become the {param} value.
    mockInvoke.mockImplementation(async (cmd: string, _args?: unknown) => {
      if (cmd === "load_context") return "reddit";
      return defaultMock(cmd);
    });

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "open google");

    await vi.waitFor(() => {
      expect(screen.getByText("Search Google")).toBeInTheDocument();
    });

    await fireEvent.keyDown(window, { key: "Enter" });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("open_url", {
        url: "https://google.com",
        param: null,
        context: "reddit",
      });
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

  it("selecting a ctx_set item sets the context, clears input, and keeps the launcher open", async () => {
    render(Page);
    await waitForReady();

    const input = screen.getByPlaceholderText("Type a command…") as HTMLInputElement;
    await typeInInput(input, "pick customer");

    await vi.waitFor(() => {
      expect(screen.getByText("Acme Corp")).toBeInTheDocument();
    });

    const row = screen.getByText("Acme Corp").closest(".result-row") as HTMLElement;
    // Reset call history (keeping the mock implementation) so we can assert
    // on exactly what selecting the item triggers, independent of startup calls.
    mockInvoke.mockClear();
    await fireEvent.click(row);

    await vi.waitFor(() => {
      // subtext ("acme-id") is used as the context value, not the title.
      expect(screen.getByText("acme-id", { selector: ".chip-label" })).toBeInTheDocument();
    });
    expect(input.value).toBe("");
    expect(mockInvoke).not.toHaveBeenCalledWith("hide_window");
    expect(mockInvoke).not.toHaveBeenCalledWith("dismiss_launcher");
    // The launcher itself is still rendered (mirrors the /ctx set builtin: input
    // clears and the launcher stays open rather than being hidden/dismissed).
    expect(screen.getByPlaceholderText("…")).toBeInTheDocument();
  });

  it("falls back to the item title as the context value when subtext is absent", async () => {
    render(Page);
    await waitForReady();

    const input = screen.getByPlaceholderText("Type a command…") as HTMLInputElement;
    await typeInInput(input, "pick vendor");

    await vi.waitFor(() => {
      expect(screen.getByText("Globex")).toBeInTheDocument();
    });

    const row = screen.getByText("Globex").closest(".result-row") as HTMLElement;
    await fireEvent.click(row);

    await vi.waitFor(() => {
      expect(screen.getByText("Globex", { selector: ".chip-label" })).toBeInTheDocument();
    });
    expect(input.value).toBe("");
  });
});

describe("script_action arg: context mode", () => {
  // A script_action command whose arg mode is "context": required, but an
  // active context satisfies the requirement. A typed suffix overrides and is
  // passed as the positional arg; with no suffix but an active context the
  // command fires with arg = null (the script reads NIMBLE_CONTEXT).
  function ctxScriptMock(context: string) {
    return async (cmd: string, _args?: unknown): Promise<unknown> => {
      switch (cmd) {
        case "load_context":
          return context;
        case "list_commands":
          return {
            commands: [
              {
                phrase: "insert env",
                title: "Insert environment",
                env: {},
                source_dir: "",
                source_file: "insert-env.yaml",
                action: {
                  type: "script_action",
                  config: { script: "env.sh", arg: "context", result_action: "paste_text" },
                },
              },
            ],
            duplicates: [],
            reserved: [],
            skipped: [],
            warnings: [],
          };
        case "run_script_action":
          return ["value"];
        case "paste_text":
          return undefined;
        default:
          return defaultMock(cmd, _args);
      }
    };
  }

  it("passes the typed suffix as the arg (explicit override)", async () => {
    mockInvoke.mockImplementation(ctxScriptMock("acme"));

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "insert env staging");

    await vi.waitFor(() => {
      expect(screen.getByText("Insert environment")).toBeInTheDocument();
    });

    await fireEvent.keyDown(window, { key: "Enter" });

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "run_script_action",
        expect.objectContaining({ arg: "staging", context: "acme" }),
      );
    });
  });

  it("fires with arg null when no suffix but a context is set", async () => {
    mockInvoke.mockImplementation(ctxScriptMock("acme"));

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "insert env");

    await vi.waitFor(() => {
      expect(screen.getByText("Insert environment")).toBeInTheDocument();
    });

    await fireEvent.keyDown(window, { key: "Enter" });

    await vi.waitFor(() => {
      // Context must NEVER be passed as the positional arg: arg is null,
      // the context reaches the script via NIMBLE_CONTEXT (the context field).
      expect(mockInvoke).toHaveBeenCalledWith(
        "run_script_action",
        expect.objectContaining({ arg: null, context: "acme" }),
      );
    });
  });

  it("does not fire when there is no suffix and no context", async () => {
    mockInvoke.mockImplementation(ctxScriptMock(""));

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "insert env");

    await vi.waitFor(() => {
      expect(screen.getByText("Insert environment")).toBeInTheDocument();
    });

    mockInvoke.mockClear();
    await fireEvent.keyDown(window, { key: "Enter" });
    // Give any async handler a chance to run before asserting non-invocation.
    await new Promise(r => setTimeout(r, 0));
    await tick();

    expect(mockInvoke).not.toHaveBeenCalledWith("run_script_action", expect.anything());
  });
});

describe("dynamic_list arg: context mode", () => {
  // A dynamic_list command whose arg mode is "context": required-like, but an
  // active context satisfies the requirement. A typed suffix IS the script's
  // arg (re-invoked as it changes, like arg:required); with no suffix but an
  // active context the script fires immediately with arg = null (it reads
  // NIMBLE_CONTEXT); with neither, the script is not invoked at all.
  function ctxDynMock(context: string) {
    return async (cmd: string, _args?: unknown): Promise<unknown> => {
      switch (cmd) {
        case "load_context":
          return context;
        case "list_commands":
          return {
            commands: [
              {
                phrase: "list envs",
                title: "List environments",
                env: {},
                source_dir: "",
                source_file: "list-envs.yaml",
                action: {
                  type: "dynamic_list",
                  config: { script: "envs.sh", arg: "context" },
                },
              },
            ],
            duplicates: [],
            reserved: [],
            skipped: [],
            warnings: [],
          };
        case "run_dynamic_list":
          return [{ title: "prod-db", subtext: "prod" }];
        default:
          return defaultMock(cmd, _args);
      }
    };
  }

  it("passes the typed suffix as the arg and re-invokes when it changes", async () => {
    mockInvoke.mockImplementation(ctxDynMock("acme"));

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "list envs staging");

    // The suffix is the script's arg (200 ms debounce), like arg:required.
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "run_dynamic_list",
        expect.objectContaining({ arg: "staging", context: "acme" }),
      );
    });

    // Changing the suffix must re-invoke the script with the new arg —
    // NOT client-side-filter the cached results (the arg:optional path).
    await typeInInput(input, "list envs qa");

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "run_dynamic_list",
        expect.objectContaining({ arg: "qa", context: "acme" }),
      );
    });
  });

  it("fires immediately with arg null on bare phrase when a context is set", async () => {
    mockInvoke.mockImplementation(ctxDynMock("acme"));

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "list envs");

    await vi.waitFor(() => {
      // Context must NEVER be passed as the positional arg: arg is null,
      // the context reaches the script via NIMBLE_CONTEXT (the context field).
      expect(mockInvoke).toHaveBeenCalledWith(
        "run_dynamic_list",
        expect.objectContaining({ arg: null, context: "acme" }),
      );
    });

    // The results are displayed like any loaded list.
    await vi.waitFor(() => {
      expect(screen.getByText("prod-db")).toBeInTheDocument();
    });
  });

  it("does not invoke the script on bare phrase when no context is set", async () => {
    mockInvoke.mockImplementation(ctxDynMock(""));

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "list envs");

    // The command row shows in normal results with the context hint.
    await vi.waitFor(() => {
      expect(screen.getByText("List environments")).toBeInTheDocument();
      expect(screen.getByText(/type a value or set a context/)).toBeInTheDocument();
    });

    // Wait past the arg:required debounce window before asserting.
    await new Promise(r => setTimeout(r, 300));
    await tick();

    expect(mockInvoke).not.toHaveBeenCalledWith("run_dynamic_list", expect.anything());
  });

  it("re-invokes the script when the active context changes (cache miss)", async () => {
    mockInvoke.mockImplementation(ctxDynMock("acme"));

    render(Page);
    await waitForReady();

    const input = screen.getByRole("textbox") as HTMLInputElement;
    await typeInInput(input, "list envs");

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "run_dynamic_list",
        expect.objectContaining({ arg: null, context: "acme" }),
      );
    });

    // Simulate a context change without retyping the input (deep link):
    // the backend emits context://changed and the frontend updates
    // activeContext. Results loaded under "acme" are now stale.
    const listenCall = mockListen.mock.calls.find(([evt]) => evt === "context://changed");
    expect(listenCall).toBeDefined();
    const handler = listenCall![1] as (event: { payload: string }) => void;

    mockInvoke.mockClear();
    handler({ payload: "globex" });
    await tick();

    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "run_dynamic_list",
        expect.objectContaining({ arg: null, context: "globex" }),
      );
    });
  });
});
