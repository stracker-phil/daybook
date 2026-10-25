/** Common Slack shortcodes as their glyphs; a workspace's custom emoji only has its name. */
const EMOJI: Record<string, string> = {
  white_check_mark: "✅",
  heavy_check_mark: "✔️",
  negative_squared_cross_mark: "❎",
  x: "❌",
  coffee: "☕",
  computer: "💻",
  zzz: "💤",
  wave: "👋",
  spiral_calendar_pad: "🗓️",
  calendar: "📆",
  date: "📅",
  palm_tree: "🌴",
  house: "🏠",
  house_with_garden: "🏡",
  car: "🚗",
  bus: "🚌",
  train: "🚆",
  airplane: "✈️",
  headphones: "🎧",
  headphone: "🎧",
  speech_balloon: "💬",
  telephone_receiver: "📞",
  video_camera: "📹",
  movie_camera: "🎥",
  no_entry: "⛔",
  octagonal_sign: "🛑",
  offline: "🛑",
  warning: "⚠️",
  information_source: "ℹ️",
  red_circle: "🔴",
  large_green_circle: "🟢",
  large_yellow_circle: "🟡",
  face_with_thermometer: "🤒",
  hamburger: "🍔",
  fork_and_knife: "🍴",
  knife_fork_plate: "🍽️",
  brain: "🧠",
  hourglass: "⌛",
  hourglass_flowing_sand: "⏳",
  focus: "🎯",
  dart: "🎯",
  rocket: "🚀",
  eyes: "👀",
  sleeping: "😴",
  sunny: "☀️",
  crescent_moon: "🌙",
};

/** `:coffee:` → `coffee`. */
export const emojiName = (code: string) => code.trim().replace(/^:|:$/g, "");

/** The glyph of a shortcode, `undefined` for a custom one. */
export const emojiGlyph = (code: string): string | undefined => EMOJI[emojiName(code)];

/** `coffee` or `:coffee` → `:coffee:`; empty stays empty. */
export function asShortcode(text: string): string {
  const name = emojiName(text);
  return name ? `:${name}:` : "";
}
