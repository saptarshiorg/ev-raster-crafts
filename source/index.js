import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { z } from "zod";

const execFileAsync = promisify(execFile);

const __dirname = dirname(fileURLToPath(import.meta.url));
const BINARY_NAME = process.platform === "win32" ? "ev-raster.exe" : "ev-raster";
const BINARY_PATH = process.env.EV_RASTER_BIN ?? join(__dirname, "target", "release", BINARY_NAME);

// Canvas edge length; must stay within the Rust engine's 1..=64 range.
const parsedSize = Number.parseInt(process.env.EV_RASTER_SIZE ?? "16", 10);
const CANVAS_SIZE = Number.isInteger(parsedSize) && parsedSize >= 1 && parsedSize <= 64 ? parsedSize : 16;
const MAX_INDEX = CANVAS_SIZE - 1;

// Buffered tool-call history, stored as compact engine commands ("cmd:args").
let history = [`size:${CANVAS_SIZE}`];

function resetHistory() {
  history = [`size:${CANVAS_SIZE}`];
}

/**
 * Serialize the whole history into the compact "cmd:args;cmd:args" wire format.
 * Compaction: redundant consecutive color selections are collapsed, so the
 * runtime argument stays small even after long sessions.
 */
function serializeHistory(extra = []) {
  const compact = [];
  for (const command of [...history, ...extra]) {
    const last = compact[compact.length - 1];
    if (command.startsWith("color:") && last === command) continue;
    compact.push(command);
  }
  return compact.join(";");
}

async function runEngine(program) {
  if (!existsSync(BINARY_PATH)) {
    throw new Error(
      `EV Raster Crafts engine not found at ${BINARY_PATH}. Run "cargo build --release" in the project folder first.`
    );
  }
  try {
    const { stdout } = await execFileAsync(BINARY_PATH, [program], {
      maxBuffer: 8 * 1024 * 1024,
      windowsHide: true,
      encoding: "utf8",
    });
    return stdout;
  } catch (error) {
    const detail = (error.stderr && String(error.stderr).trim()) || error.message;
    throw new Error(detail);
  }
}

function textResult(text, isError = false) {
  return { content: [{ type: "text", text }], ...(isError ? { isError: true } : {}) };
}

/**
 * Validate a command against the engine by dry-running history + command.
 * Only commands the engine accepts are committed to the buffered history.
 */
async function commit(command, message) {
  try {
    await runEngine(serializeHistory([command, "render"]));
  } catch (error) {
    return textResult(`EV Raster Crafts rejected the command: ${error.message}`, true);
  }
  history.push(command);
  return textResult(message);
}

const coord = z.number().int().min(0).max(MAX_INDEX);
const colorSchema = z
  .string()
  .regex(
    /^(#?[0-9a-fA-F]{3}|#?[0-9a-fA-F]{6}|white|black|red|blue|green|yellow|orange|purple|brown)$/i,
    "Use a hex color like #FF0000 or a name: white, black, red, blue, green, yellow, orange, purple, brown"
  );

const server = new McpServer({ name: "ev-raster", version: "0.1.0" });

server.tool(
  "ev_raster_draw_pixel",
  `Pencil: paint one pixel on the ${CANVAS_SIZE}x${CANVAS_SIZE} canvas (x = column, y = row, origin top-left, 0-${MAX_INDEX}). The color becomes the primary color.`,
  { x: coord, y: coord, color: colorSchema },
  async ({ x, y, color }) => commit(`pixel:${x},${y},${color}`, `Pixel (${x}, ${y}) set to ${color}.`)
);

server.tool(
  "ev_raster_flood_fill",
  "Fill bucket: flood-fill (4-way) the connected region containing (x, y) with a color.",
  { x: coord, y: coord, color: colorSchema },
  async ({ x, y, color }) => commit(`fill:${x},${y},${color}`, `Region at (${x}, ${y}) filled with ${color}.`)
);

server.tool(
  "ev_raster_transform",
  "Transform the whole canvas: rotate_90 (rotate right 90), rotate_180, rotate_270 (rotate left 90), flip_horizontal, flip_vertical.",
  {
    operation: z.enum(["rotate_90", "rotate_180", "rotate_270", "flip_horizontal", "flip_vertical"]),
  },
  async ({ operation }) => {
    const commands = {
      rotate_90: "rotate:90",
      rotate_180: "rotate:180",
      rotate_270: "rotate:270",
      flip_horizontal: "flip:horizontal",
      flip_vertical: "flip:vertical",
    };
    return commit(commands[operation], `Applied ${operation}.`);
  }
);

server.tool(
  "ev_raster_draw_line",
  "Brush: draw a continuous stroke between two points (all cells in between are painted).",
  { x0: coord, y0: coord, x1: coord, y1: coord, color: colorSchema },
  async ({ x0, y0, x1, y1, color }) =>
    commit(`line:${x0},${y0},${x1},${y1},${color}`, `Line (${x0}, ${y0}) to (${x1}, ${y1}) drawn in ${color}.`)
);

server.tool(
  "ev_raster_erase",
  "Eraser: overwrite a pixel with the white background.",
  { x: coord, y: coord },
  async ({ x, y }) => commit(`erase:${x},${y}`, `Pixel (${x}, ${y}) erased.`)
);

server.tool(
  "ev_raster_set_color",
  "Set the primary color without drawing; it is added to the palette history.",
  { color: colorSchema },
  async ({ color }) => commit(`color:${color}`, `Primary color set to ${color}.`)
);

server.tool(
  "ev_raster_reset",
  "Start a fresh blank canvas and clear the buffered tool call history.",
  {},
  async () => {
    resetHistory();
    return textResult("Canvas reset to a blank white grid.");
  }
);

server.tool(
  "ev_raster_view_canvas",
  "Render the current canvas by replaying the buffered tool call history through the native Rust engine, and return the emoji grid.",
  {},
  async () => {
    try {
      const output = await runEngine(serializeHistory(["render"]));
      return textResult(output);
    } catch (error) {
      return textResult(`Render failed: ${error.message}`, true);
    }
  }
);

const transport = new StdioServerTransport();
await server.connect(transport);
