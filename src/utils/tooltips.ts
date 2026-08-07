const TOOLTIP_SELECTOR = "[data-tooltip], [title]";
const TOOLTIP_DELAY_MS = 350;
const TOOLTIP_HIDE_MS = 100;
const TOOLTIP_OFFSET_PX = 8;
const VIEWPORT_PADDING_PX = 8;

type TooltipTarget = HTMLElement;

interface TooltipController {
  destroy: () => void;
}

let tooltipId = 0;

function asTooltipTarget(node: EventTarget | null): TooltipTarget | null {
  if (!(node instanceof HTMLElement)) return null;
  return node.closest<TooltipTarget>(TOOLTIP_SELECTOR);
}

function migrateTitle(target: TooltipTarget): string {
  const title = target.getAttribute("title")?.trim();
  if (title) {
    target.setAttribute("data-tooltip", title);
    target.removeAttribute("title");
  }
  return target.getAttribute("data-tooltip")?.trim() ?? "";
}

function readTooltipDelay(): number {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue("--tooltip-delay")
    .trim();
  const amount = Number.parseFloat(value);
  if (!Number.isFinite(amount)) return TOOLTIP_DELAY_MS;
  return value.endsWith("s") && !value.endsWith("ms") ? amount * 1000 : amount;
}

/** Replace native title popups with a theme-aware tooltip layer. */
export function installTooltips(): TooltipController {
  let activeTarget: TooltipTarget | null = null;
  let tooltip: HTMLDivElement | null = null;
  let showTimer: number | undefined;
  let scheduledTarget: TooltipTarget | null = null;
  let hideTimer: number | undefined;
  let moveRaf: number | undefined;
  let pointerX: number | null = null;
  let pointerY: number | null = null;
  let describedByBeforeShow: string | null = null;

  const cancelMoveRaf = () => {
    if (moveRaf !== undefined) window.cancelAnimationFrame(moveRaf);
    moveRaf = undefined;
  };

  const clearShowTimer = () => {
    if (showTimer !== undefined) window.clearTimeout(showTimer);
    showTimer = undefined;
    scheduledTarget = null;
  };

  const clearHideTimer = () => {
    if (hideTimer !== undefined) window.clearTimeout(hideTimer);
    hideTimer = undefined;
  };

  const reposition = () => {
    if (!activeTarget || !tooltip) return;

    const targetRect = activeTarget.getBoundingClientRect();
    const tooltipRect = tooltip.getBoundingClientRect();
    // Anchor to the pointer when available (natural for hover, and fixes wide
    // triggers like the alias button whose center is far from the edit icon),
    // else fall back to the element's center (focus-driven shows).
    const anchorX = pointerX ?? targetRect.left + targetRect.width / 2;
    const anchorY = pointerY ?? targetRect.top + targetRect.height / 2;
    const canPlaceAbove = anchorY - tooltipRect.height - TOOLTIP_OFFSET_PX >= VIEWPORT_PADDING_PX;
    const top = canPlaceAbove
      ? anchorY - tooltipRect.height - TOOLTIP_OFFSET_PX
      : anchorY + TOOLTIP_OFFSET_PX;
    const left = Math.min(
      Math.max(anchorX - tooltipRect.width / 2, VIEWPORT_PADDING_PX),
      Math.max(VIEWPORT_PADDING_PX, window.innerWidth - tooltipRect.width - VIEWPORT_PADDING_PX),
    );

    tooltip.dataset.placement = canPlaceAbove ? "top" : "bottom";
    tooltip.style.top = `${Math.min(Math.max(VIEWPORT_PADDING_PX, top), window.innerHeight - tooltipRect.height - VIEWPORT_PADDING_PX)}px`;
    tooltip.style.left = `${left}px`;
  };

  const removeTooltip = () => {
    clearHideTimer();
    cancelMoveRaf();
    window.removeEventListener("resize", reposition);
    document.removeEventListener("scroll", reposition, true);
    tooltip?.remove();
    tooltip = null;
    if (activeTarget) {
      if (describedByBeforeShow === null) {
        activeTarget.removeAttribute("aria-describedby");
      } else {
        activeTarget.setAttribute("aria-describedby", describedByBeforeShow);
      }
    }
    describedByBeforeShow = null;
    activeTarget = null;
  };

  const hideTooltip = () => {
    // Only cancel a pending show when it belongs to the target being hidden.
    // Moving A→B schedules B's show (pointerover) before A's fade-out path
    // runs (pointerout / frame-based move check); cancelling that timer here
    // would leave B without a tooltip until the pointer leaves and re-enters.
    if (scheduledTarget === activeTarget) clearShowTimer();
    if (!tooltip) return;
    tooltip.dataset.visible = "false";
    hideTimer = window.setTimeout(removeTooltip, TOOLTIP_HIDE_MS);
  };

  const showTooltip = (target: TooltipTarget) => {
    clearShowTimer();
    clearHideTimer();
    removeTooltip();

    const text = migrateTitle(target);
    if (!text) return;

    activeTarget = target;
    describedByBeforeShow = target.getAttribute("aria-describedby");
    tooltip = document.createElement("div");
    tooltip.className = "app-tooltip";
    tooltip.id = `app-tooltip-${++tooltipId}`;
    tooltip.setAttribute("role", "tooltip");
    tooltip.dataset.visible = "false";
    tooltip.textContent = text;
    document.body.appendChild(tooltip);
    target.setAttribute(
      "aria-describedby",
      [describedByBeforeShow, tooltip.id].filter(Boolean).join(" "),
    );

    reposition();
    window.addEventListener("resize", reposition);
    document.addEventListener("scroll", reposition, true);
    requestAnimationFrame(() => {
      if (tooltip && activeTarget === target) tooltip.dataset.visible = "true";
    });
  };

const scheduleShow = (target: TooltipTarget) => {
    clearHideTimer();
    // Already visibly shown for this target — nothing to do.
    if (activeTarget === target && tooltip && tooltip.dataset.visible === "true") return;
    // Tooltip is mounted but mid-fade for this same target (e.g. moving to
    // button B, then back to A before B's delay elapses). The stale target is
    // still active so the generic branch would early-return and the tooltip
    // would never come back — re-show it immediately instead.
    if (activeTarget === target && tooltip) {
      showTooltip(target);
      return;
    }
    // New/different target (or nothing shown): don't reset the deadline for the
    // same target — fast micro-moves across a target's children (icon/text/button
    // edge) fire pointerover repeatedly, which would otherwise keep pushing the
    // timer out and the tooltip would rarely appear on quick hovers. Only re-arm
    // when the *intended* target actually changes.
    if (scheduledTarget === target) return;
    clearShowTimer();
    scheduledTarget = target;
    showTimer = window.setTimeout(() => {
      scheduledTarget = null;
      showTooltip(target);
    }, readTooltipDelay());
  };

  const onPointerOver = (event: PointerEvent) => {
    pointerX = event.clientX;
    pointerY = event.clientY;
    const target = asTooltipTarget(event.target);
    if (!target || (event.relatedTarget instanceof Node && target.contains(event.relatedTarget))) return;
    scheduleShow(target);
  };

  const onPointerOut = (event: PointerEvent) => {
    const target = asTooltipTarget(event.target);
    if (!target || (event.relatedTarget instanceof Node && target.contains(event.relatedTarget))) return;
    if (activeTarget === target) hideTooltip();
    else clearShowTimer();
  };

  // Fallback hide: pointerout/relatedTarget can misfire on tiny or clipped
  // targets (e.g. a 14px tag-remove button), leaving the tooltip stuck. Once a
  // tooltip is up, re-check on pointermove that the pointer is actually still
  // over the anchored element; if not, hide it. elementFromPoint honours the
  // tooltip's pointer-events: none, so the tooltip itself never counts as "over".
  const scheduleMoveCheck = (event: PointerEvent) => {
    if (!activeTarget || !tooltip) return;
    pointerX = event.clientX;
    pointerY = event.clientY;
    cancelMoveRaf();
    moveRaf = window.requestAnimationFrame(() => {
      moveRaf = undefined;
      let over = false;
      try {
        const el = document.elementFromPoint(event.clientX, event.clientY);
        over = !!el && !!activeTarget && activeTarget.contains(el);
      } catch {
        over = false;
      }
      if (!over) {
        hideTooltip();
      } else {
        reposition();
      }
    });
  };

  const onPointerMove = (event: PointerEvent) => {
    // Re-hide checks before the show delay has elapsed are unnecessary — the
    // pointerout path already clears pending shows when the pointer leaves.
    if (!tooltip) return;
    scheduleMoveCheck(event);
  };

  const onFocusIn = (event: FocusEvent) => {
    const target = asTooltipTarget(event.target);
    if (!target) return;
    // Focus-driven shows have no pointer position; anchor to the element center.
    pointerX = null;
    pointerY = null;
    scheduleShow(target);
  };

  const onFocusOut = (event: FocusEvent) => {
    const target = asTooltipTarget(event.target);
    if (!target || (event.relatedTarget instanceof Node && target.contains(event.relatedTarget))) return;
    if (activeTarget === target) hideTooltip();
    else clearShowTimer();
  };

  const migrateExistingTitles = () => {
    document.querySelectorAll<HTMLElement>("[title]").forEach(migrateTitle);
  };

  const observer = new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === "attributes" && record.target instanceof HTMLElement) {
        migrateTitle(record.target);
      }
      record.addedNodes.forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        if (node.hasAttribute("title")) migrateTitle(node);
        node.querySelectorAll<HTMLElement>("[title]").forEach(migrateTitle);
      });
    }
  });

  migrateExistingTitles();
  observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["title"] });
  document.addEventListener("pointerover", onPointerOver);
  document.addEventListener("pointerout", onPointerOut);
  document.addEventListener("pointermove", onPointerMove);
  document.addEventListener("focusin", onFocusIn);
  document.addEventListener("focusout", onFocusOut);

  return {
    destroy: () => {
      clearShowTimer();
      observer.disconnect();
      document.removeEventListener("pointerover", onPointerOver);
      document.removeEventListener("pointerout", onPointerOut);
      document.removeEventListener("pointermove", onPointerMove);
      document.removeEventListener("focusin", onFocusIn);
      document.removeEventListener("focusout", onFocusOut);
      removeTooltip();
    },
  };
}
