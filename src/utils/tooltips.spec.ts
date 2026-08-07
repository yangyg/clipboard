import { afterEach, describe, expect, it, vi } from "vitest";
import { installTooltips } from "./tooltips";

describe("tooltips", () => {
  let destroy: (() => void) | undefined;

  afterEach(() => {
    destroy?.();
    destroy = undefined;
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("replaces native title text and exposes the custom tooltip on focus", () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<button title="Open settings">Settings</button>';
    const button = document.querySelector("button")!;
    destroy = installTooltips().destroy;

    expect(button.getAttribute("title")).toBeNull();
    expect(button.dataset.tooltip).toBe("Open settings");

    button.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    vi.advanceTimersByTime(350);

    const tooltip = document.querySelector(".app-tooltip");
    expect(tooltip?.textContent).toBe("Open settings");
    expect(tooltip?.getAttribute("role")).toBe("tooltip");
    expect(button.getAttribute("aria-describedby")).toBe(tooltip?.id);
  });

  it("handles titles added after installation", async () => {
    document.body.innerHTML = "";
    destroy = installTooltips().destroy;
    const button = document.createElement("button");
    button.title = "Dynamic help";
    document.body.appendChild(button);
    await Promise.resolve();

    expect(button.getAttribute("title")).toBeNull();
    expect(button.dataset.tooltip).toBe("Dynamic help");
  });

  it("hides the tooltip when the pointer leaves the target", () => {
    vi.useFakeTimers();
    // jsdom's requestAnimationFrame is not advanced by the fake timer clock, so
    // drive it synchronously to exercise the frame-based hide check.
    window.requestAnimationFrame = ((cb) => {
      cb(0);
      return 0;
    }) as typeof window.requestAnimationFrame;
    window.cancelAnimationFrame = (() => {}) as typeof window.cancelAnimationFrame;

    document.body.innerHTML = '<button title="Remove">x</button>';
    const button = document.querySelector("button")!;
    const elementFromPoint = vi.fn<() => Element | null>(() => button);
    document.elementFromPoint = elementFromPoint as typeof document.elementFromPoint;
    destroy = installTooltips().destroy;

    // Pointer enters the button; tooltip should anchor near the pointer (like a
    // wide alias button where element-centre would be far from the icon).
    button.dispatchEvent(new MouseEvent("pointerover", { bubbles: true, clientX: 50, clientY: 30 }));
    vi.advanceTimersByTime(400);
    const tooltip = document.querySelector<HTMLElement>(".app-tooltip");
    expect(tooltip).not.toBeNull();
    expect(tooltip!.style.left).toBe("50px");

    // Pointer moves off the small button: nothing is "over" it anymore.
    elementFromPoint.mockReturnValue(null);
    button.dispatchEvent(new MouseEvent("pointermove", { bubbles: true, clientX: 2, clientY: 2 }));
    // Frame-based hide check, then the fade-removal timer.
    vi.advanceTimersByTime(20);
    vi.advanceTimersByTime(300);

    expect(document.querySelector(".app-tooltip")).toBeNull();
    expect(button.hasAttribute("aria-describedby")).toBe(false);
  });

  it("shows when the pointer moves fast across the target without resetting the deadline", () => {
    vi.useFakeTimers();
    window.requestAnimationFrame = ((cb) => {
      cb(0);
      return 0;
    }) as typeof window.requestAnimationFrame;
    window.cancelAnimationFrame = (() => {}) as typeof window.cancelAnimationFrame;

    document.body.innerHTML = '<button title="Quick"><span>icon</span></button>';
    const button = document.querySelector("button")!;
    destroy = installTooltips().destroy;

    // Fast hops between the button's children must not restart the 350ms timer.
    button.dispatchEvent(new MouseEvent("pointerover", { bubbles: true }));
    vi.advanceTimersByTime(200);
    const icon = button.querySelector("span")!;
    icon.dispatchEvent(new MouseEvent("pointerover", { bubbles: true }));
    vi.advanceTimersByTime(160); // 360ms since first enter > 350ms deadline, not deferred by the second hop

    expect(document.querySelector(".app-tooltip")).not.toBeNull();
  });

  it("re-shows when moving back to a button before the other target's delay elapses", () => {
    vi.useFakeTimers();
    window.requestAnimationFrame = ((cb) => {
      cb(0);
      return 0;
    }) as typeof window.requestAnimationFrame;
    window.cancelAnimationFrame = (() => {}) as typeof window.cancelAnimationFrame;

    document.body.innerHTML = '<button id="a" title="A">a</button><button id="b" title="B">b</button>';
    const a = document.querySelector<HTMLElement>("#a")!;
    const b = document.querySelector<HTMLElement>("#b")!;
    const elementFromPoint = vi.fn<() => Element | null>(() => a);
    document.elementFromPoint = elementFromPoint as typeof document.elementFromPoint;
    destroy = installTooltips().destroy;

    // Hover A → tooltip fully shown.
    a.dispatchEvent(new MouseEvent("pointerover", { bubbles: true }));
    vi.advanceTimersByTime(400);
    expect(document.querySelector<HTMLElement>(".app-tooltip")!.dataset.visible).toBe("true");

    // Move to B: A fades but its (invisible) tooltip + activeTarget linger because
    // scheduleShow(B) cancels the fade-removal timer before it runs.
    elementFromPoint.mockReturnValue(b);
    a.dispatchEvent(new MouseEvent("pointerout", { bubbles: true, relatedTarget: b }));
    b.dispatchEvent(new MouseEvent("pointerover", { bubbles: true, relatedTarget: a }));
    vi.advanceTimersByTime(30);

    // Move back to A inside B's delay window — the tooltip must come straight back.
    elementFromPoint.mockReturnValue(a);
    b.dispatchEvent(new MouseEvent("pointerout", { bubbles: true, relatedTarget: a }));
    a.dispatchEvent(new MouseEvent("pointerover", { bubbles: true, relatedTarget: b }));

    const tooltip = document.querySelector<HTMLElement>(".app-tooltip");
    expect(tooltip).not.toBeNull();
    expect(tooltip!.dataset.visible).toBe("true");
  });

  it("shows the next target's tooltip when moving directly from A to B", () => {
    vi.useFakeTimers();
    window.requestAnimationFrame = ((cb) => {
      cb(0);
      return 0;
    }) as typeof window.requestAnimationFrame;
    window.cancelAnimationFrame = (() => {}) as typeof window.cancelAnimationFrame;

    document.body.innerHTML = '<button id="a" title="A">a</button><button id="b" title="B">b</button>';
    const a = document.querySelector<HTMLElement>("#a")!;
    const b = document.querySelector<HTMLElement>("#b")!;
    const elementFromPoint = vi.fn<() => Element | null>(() => a);
    document.elementFromPoint = elementFromPoint as typeof document.elementFromPoint;
    destroy = installTooltips().destroy;

    // Hover A → tooltip fully shown.
    a.dispatchEvent(new MouseEvent("pointerover", { bubbles: true }));
    vi.advanceTimersByTime(400);
    expect(document.querySelector<HTMLElement>(".app-tooltip")!.dataset.visible).toBe("true");

    // Move directly to B: A fades, B's show is scheduled, then the frame-based
    // move check runs while B's delay is still pending. It must not cancel it.
    elementFromPoint.mockReturnValue(b);
    a.dispatchEvent(new MouseEvent("pointerout", { bubbles: true, relatedTarget: b }));
    b.dispatchEvent(new MouseEvent("pointerover", { bubbles: true, relatedTarget: a }));
    b.dispatchEvent(new MouseEvent("pointermove", { bubbles: true, clientX: 200, clientY: 30 }));
    vi.advanceTimersByTime(400);

    const tooltip = document.querySelector<HTMLElement>(".app-tooltip");
    expect(tooltip?.textContent).toBe("B");
    expect(tooltip?.dataset.visible).toBe("true");
  });
});
