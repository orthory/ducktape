import type { SearchProvider } from "@cloudflare/nimbus-docs/types";

export interface SearchConfig {
  input: HTMLInputElement;
  resultsContainer: HTMLElement;
  emptyState: HTMLElement;
  provider: SearchProvider;
  onNavigate?: () => void;
}

export interface SearchInstance {
  reset(): Promise<void>;
  destroy(): void;
}

export function initSearch(config: SearchConfig): SearchInstance {
  const { input, resultsContainer, emptyState, provider, onNavigate } = config;

  let initialized = false;
  let activeIndex = -1;
  let resultIdCounter = 0;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  let activeController: AbortController | undefined;

  function getOptions(): HTMLElement[] {
    return Array.from(resultsContainer.querySelectorAll<HTMLElement>("[role='option']"));
  }

  function updateActive(newIndex: number): void {
    const options = getOptions();
    if (options.length === 0) {
      activeIndex = -1;
      input.removeAttribute("aria-activedescendant");
      return;
    }
    activeIndex = Math.max(-1, Math.min(newIndex, options.length - 1));
    options.forEach((option, index) => {
      if (index === activeIndex) {
        option.setAttribute("data-highlighted", "");
        option.scrollIntoView({ block: "nearest" });
        input.setAttribute("aria-activedescendant", option.id);
      } else {
        option.removeAttribute("data-highlighted");
      }
    });
    if (activeIndex < 0) input.removeAttribute("aria-activedescendant");
  }

  function clearResults(): void {
    for (const result of resultsContainer.querySelectorAll("[role='option']")) result.remove();
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
  }

  function buildOption(title: string, href: string, snippet?: string, secondary = false): HTMLAnchorElement {
    const option = document.createElement("a");
    option.id = `search-result-${resultIdCounter++}`;
    option.setAttribute("role", "option");
    option.setAttribute("aria-label", title);
    option.tabIndex = -1;
    option.href = href;
    option.className = "block min-h-11 rounded-lg px-2 py-2 text-foreground no-underline transition-colors hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-[-2px] data-[highlighted]:bg-accent";

    const label = document.createElement("span");
    label.className = secondary
      ? "block truncate text-xs text-muted-foreground"
      : "block truncate text-sm font-medium";
    label.textContent = title;
    option.appendChild(label);

    if (snippet) {
      const excerpt = document.createElement("p");
      excerpt.className = "mt-1 line-clamp-2 text-xs leading-relaxed text-muted-foreground";
      excerpt.innerHTML = snippet;
      option.appendChild(excerpt);
    }

    option.addEventListener("click", () => onNavigate?.());

    return option;
  }

  async function ensureInitialized(): Promise<boolean> {
    if (initialized) return true;
    try {
      await provider.init?.();
      initialized = true;
      return true;
    } catch {
      emptyState.textContent = "Search is available after a production build.";
      return false;
    }
  }

  async function runSearch(query: string): Promise<void> {
    activeController?.abort();
    activeController = new AbortController();
    const signal = activeController.signal;

    emptyState.style.display = "";
    emptyState.textContent = "Searching…";
    clearResults();

    if (!(await ensureInitialized()) || signal.aborted) return;

    try {
      const results = await provider.search(query, { signal });
      if (signal.aborted) return;

      clearResults();
      activeIndex = -1;

      if (results.length === 0) {
        emptyState.style.display = "";
        emptyState.textContent = "No results found.";
        return;
      }

      emptyState.style.display = "none";
      input.setAttribute("aria-expanded", "true");
      for (const result of results) {
        resultsContainer.appendChild(buildOption(result.title, result.url, result.snippet));
        for (const sub of result.subResults?.slice(0, 3) ?? []) {
          resultsContainer.appendChild(buildOption(sub.title, sub.url, undefined, true));
        }
      }
    } catch {
      if (signal.aborted) return;
      clearResults();
      emptyState.style.display = "";
      emptyState.textContent = "Search is temporarily unavailable.";
    }
  }

  function handleInput(): void {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      const query = input.value.trim();
      if (!query) {
        activeController?.abort();
        clearResults();
        emptyState.style.display = "";
        emptyState.textContent = "Type to search…";
        return;
      }
      void runSearch(query);
    }, 150);
  }

  function handleKeydown(event: KeyboardEvent): void {
    const options = getOptions();
    if (event.key === "ArrowDown") {
      event.preventDefault();
      updateActive(activeIndex + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      updateActive(activeIndex - 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      updateActive(0);
    } else if (event.key === "End") {
      event.preventDefault();
      updateActive(options.length - 1);
    } else if (event.key === "Enter" && activeIndex >= 0) {
      event.preventDefault();
      options[activeIndex]?.click();
    }
  }

  input.addEventListener("input", handleInput);
  input.closest("dialog")?.addEventListener("keydown", handleKeydown);

  return {
    async reset() {
      activeController?.abort();
      if (debounceTimer) clearTimeout(debounceTimer);
      input.value = "";
      input.focus();
      activeIndex = -1;
      clearResults();
      emptyState.style.display = "";
      emptyState.textContent = "Type to search…";
      await ensureInitialized();
    },
    destroy() {
      activeController?.abort();
      if (debounceTimer) clearTimeout(debounceTimer);
      input.removeEventListener("input", handleInput);
      input.closest("dialog")?.removeEventListener("keydown", handleKeydown);
    },
  };
}
