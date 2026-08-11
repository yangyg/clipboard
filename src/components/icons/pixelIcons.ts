/**
 * Pixel-art icon set for the Pixel theme family (像素 / 像素·浅).
 *
 * Every icon is hand-placed on a shared 16×16 integer grid so the "pixels"
 * land on exact block boundaries at any render size (0.5px never occurs).
 * Icons render as solid `currentColor` rectangles with `shape-rendering:
 * crispEdges`, so they keep the retro sprite look at both 13px list rows and
 * 36px empty states.
 *
 * This is a *partial* set — any AppIconName without an entry falls back to the
 * clean Lucide icon in `AppIcon.vue`, so the map never blocks new call sites.
 */

import { defineComponent, h } from "vue";
import type { Component } from "vue";
import type { AppIconName } from "./AppIcon.vue";

interface PixelCells {
  /** Cell rects "x,y,w,h" drawn when `fill` is "none" (the outline/hollow pass). */
  cells: readonly string[];
  /** Optional blockier solid pass used when `fill` is currentColor (active star/pin). */
  filled?: readonly string[];
}

const GEAR = [
  "6,6,4,4",
  "5,3,2,2",
  "9,3,2,2",
  "3,5,2,2",
  "3,9,2,2",
  "5,11,2,2",
  "9,11,2,2",
  "11,5,2,2",
  "11,9,2,2",
];

const CLOUD_BASE = ["3,3,10,2", "2,5,12,2", "2,7,12,3"];

const CELLS: Record<string, PixelCells> = {
  clipboard: { cells: ["6,1,4,3", "4,4,8,11"] },
  search: { cells: ["4,3,7,7", "9,9,4,4"] },
  settings: { cells: GEAR },
  /* Star: hollow = single-pix outline, filled = solid sprite. */
  star: {
    cells: [
      "7,2,2,2",
      "5,3,6,1",
      "4,4,8,1",
      "3,5,4,1",
      "9,5,4,1",
      "2,6,3,2",
      "11,6,3,2",
      "3,8,3,2",
      "10,8,3,2",
      "3,10,3,2",
      "10,10,3,2",
      "5,11,2,1",
      "9,11,2,1",
      "6,12,4,2",
    ],
    filled: [
      "6,2,4,2",
      "5,3,6,1",
      "4,4,8,1",
      "3,5,10,2",
      "2,7,12,3",
      "3,10,10,2",
      "4,11,8,1",
      "5,12,6,1",
    ],
  },
  pin: { cells: ["4,2,8,2", "3,4,10,3", "5,7,6,1", "6,8,2,5", "5,13,6,1"] },
  trash: { cells: ["2,2,12,2", "4,4,8,10"] },
  paste: { cells: ["6,2,3,5", "2,7,7,3", "2,11,12,2"] },
  close: {
    cells: [
      "3,3,2,2",
      "5,5,2,2",
      "7,7,2,2",
      "9,9,2,2",
      "9,3,2,2",
      "7,5,2,2",
      "5,7,2,2",
      "3,9,2,2",
    ],
  },
  back: { cells: ["3,6,9,3", "9,2,3,2", "9,11,3,2"] },
  plus: { cells: ["6,3,4,10", "3,6,10,4"] },
  copy: { cells: ["6,2,7,9", "3,5,8,9"] },
  check: { cells: ["2,9,5,2", "7,7,3,2", "9,5,3,2", "11,3,3,2"] },
  pause: { cells: ["3,3,3,10", "10,3,3,10"] },
  play: { cells: ["3,6,2,4", "5,5,2,6", "7,4,2,8", "9,3,2,10"] },
  list: {
    cells: ["3,3,2,2", "8,3,6,2", "3,7,2,2", "8,7,6,2", "3,11,2,2", "8,11,6,2"],
  },
  grid: { cells: ["3,3,4,4", "9,3,4,4", "3,9,4,4", "9,9,4,4"] },
  arrowUp: { cells: ["7,2,2,6", "3,8,10,3"] },
  tag: { cells: ["3,4,9,8", "11,6,2,4"] },
  warning: {
    cells: ["7,2,2,2", "5,4,6,2", "4,6,8,2", "3,8,10,2", "3,10,10,2", "5,12,6,2"],
  },
  zap: {
    cells: [
      "8,2,3,3",
      "7,5,3,3",
      "6,8,3,2",
      "3,9,5,2",
      "4,11,4,2",
      "5,13,3,2",
    ],
  },
  sparkles: { cells: ["7,2,2,8", "3,5,8,2", "11,3,2,2", "3,11,2,2"] },
  batch: {
    cells: [
      "3,3,10,2",
      "3,11,10,2",
      "3,3,2,10",
      "11,3,2,10",
      "5,9,3,2",
      "8,7,3,2",
      "11,5,2,2",
    ],
  },
  restore: {
    cells: ["6,6,4,4", "3,3,3,3", "10,3,3,3", "3,10,3,3", "10,10,3,3", "13,2,2,2"],
  },
  refresh: {
    cells: ["6,6,4,4", "3,3,3,3", "10,3,3,3", "3,10,3,3", "10,10,3,3", "13,2,2,2"],
  },
  type: { cells: ["3,3,10,2", "7,5,2,9"] },
  link: {
    cells: ["3,4,5,2", "3,10,5,2", "3,4,2,8", "9,6,5,2", "9,12,5,2", "11,6,2,8"],
  },
  image: {
    cells: [
      "2,2,13,2",
      "2,13,13,1",
      "2,2,2,12",
      "13,2,1,12",
      "4,10,5,2",
      "7,8,4,2",
      "5,4,2,2",
    ],
  },
  file: { cells: ["3,2,10,12"] },
  code: {
    cells: [
      "2,6,3,3",
      "5,4,3,2",
      "5,10,3,2",
      "11,6,3,3",
      "8,4,3,2",
      "8,10,3,2",
      "10,2,2,2",
      "9,5,2,1",
      "8,8,2,1",
    ],
  },
  keyboard: { cells: ["3,4,10,2", "3,7,10,2", "3,10,3,2", "10,10,3,2"] },
  history: {
    cells: [
      "3,3,10,2",
      "3,11,10,2",
      "3,3,2,10",
      "11,3,2,10",
      "7,5,2,3",
      "7,7,3,2",
      "13,2,2,2",
      "12,5,2,1",
    ],
  },
  palette: { cells: ["4,4,8,2", "4,12,8,2", "4,4,2,10", "10,4,2,10"] },
  shield: { cells: ["4,2,8,2", "3,4,10,2", "3,6,10,3", "4,9,8,4", "5,13,6,2", "6,15,4,1"] },
  stats: { cells: ["3,9,3,6", "7,5,3,10", "11,3,3,13"] },
  package: { cells: ["3,4,10,2", "3,6,10,9", "6,8,4,4"] },
  info: {
    cells: ["3,3,10,2", "3,11,10,2", "3,3,2,10", "11,3,2,10", "7,5,2,2", "7,8,2,4"],
  },
  help: { cells: ["7,2,2,2", "5,4,6,2", "4,6,3,2", "9,6,3,2", "7,8,2,4"] },
  moon: { cells: ["4,3,8,2", "3,5,7,2", "3,7,6,2", "4,9,5,3", "6,11,3,2"] },
  sun: {
    cells: [
      "5,5,6,6",
      "7,2,2,2",
      "2,7,2,2",
      "12,7,2,2",
      "7,12,2,2",
      "4,3,2,2",
      "10,3,2,2",
      "4,11,2,2",
      "10,11,2,2",
    ],
  },
  circle: { cells: ["4,3,8,2", "3,5,10,2", "3,7,10,2", "3,9,10,2", "4,11,8,2"] },
  monitor: { cells: ["2,3,12,8", "6,11,4,3", "4,14,8,2"] },
  component: { cells: ["3,4,5,5", "8,7,5,5", "6,6,2,2", "9,10,2,2"] },
  eye: { cells: ["4,4,8,2", "3,6,10,3", "4,9,8,2", "7,7,2,2"] },
  eyeOff: {
    cells: [
      "4,4,8,2",
      "3,6,10,3",
      "4,9,8,2",
      "11,2,2,2",
      "9,4,2,2",
      "7,6,2,2",
      "5,8,2,2",
      "3,10,2,2",
    ],
  },
  settings2: { cells: GEAR },
  cloud: { cells: CLOUD_BASE },
  cloudUpload: { cells: [...CLOUD_BASE, "7,10,2,2", "5,11,6,2"] },
  cloudDownload: { cells: ["3,3,10,2", "2,5,12,2", "2,7,12,2", "7,9,2,2", "5,10,6,2"] },
  edit: {
    cells: [
      "2,14,3,2",
      "4,12,3,2",
      "6,10,3,2",
      "8,8,3,2",
      "10,6,3,2",
      "12,4,3,2",
    ],
  },
  pencilOff: {
    cells: [
      "2,14,3,2",
      "4,12,3,2",
      "6,10,3,2",
      "8,8,3,2",
      "10,6,3,2",
      "12,4,3,2",
      "11,2,2,2",
      "9,4,2,2",
      "7,6,2,2",
      "5,8,2,2",
      "3,10,2,2",
    ],
  },
};

function makeIcon(def: PixelCells): Component {
  return defineComponent({
    name: "PixelIcon",
    props: {
      size: { type: [Number, String], default: 16 },
      fill: { type: String, default: "none" },
    },
    setup(props) {
      return () => {
        const filled = props.fill !== "none";
        const cells = filled ? (def.filled ?? def.cells) : def.cells;
        return h(
          "svg",
          {
            class: "app-icon",
            width: props.size,
            height: props.size,
            viewBox: "0 0 16 16",
            "shape-rendering": "crispEdges",
            "aria-hidden": "true",
            fill: "currentColor",
          },
          cells.map((cell) => {
            const [x, y, w, h2] = cell.split(",").map(Number);
            return h("rect", { x, y, width: w, height: h2 });
          }),
        );
      };
    },
  });
}

export const PIXEL_ICONS: Partial<Record<AppIconName, Component>> = Object.fromEntries(
  Object.keys(CELLS).map((name) => [name, makeIcon(CELLS[name])]),
) as Partial<Record<AppIconName, Component>>;