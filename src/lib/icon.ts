// Game icons: any image the WebView can read (PNG, JPG, WebP, GIF, BMP, AVIF...) becomes a square 128px
// PNG before it's saved, so it's small and the notification cards (drawn with GDI+, which can't read WebP)
// can always show it.

export const ICON_SIZE = 128;
const MAX_SOURCE_BYTES = 20_000_000;

/** The centred square of a width × height image: what to crop so the icon fills its square. */
export function squareCrop(width: number, height: number): { x: number; y: number; size: number } {
  const size = Math.min(width, height);
  return { x: Math.floor((width - size) / 2), y: Math.floor((height - size) / 2), size };
}

/** An image's bytes (a picked file or a download) as a 128px PNG. Throws a readable message if it isn't one. */
export async function toIconPng(image: Blob): Promise<Uint8Array> {
  if (image.size > MAX_SOURCE_BYTES) throw new Error("That image is over 20 MB. Please pick a smaller one.");
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(image);
  } catch {
    throw new Error("That isn't an image this app can read.");
  }
  const { x, y, size } = squareCrop(bitmap.width, bitmap.height);
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = ICON_SIZE;
  const context = canvas.getContext("2d")!;
  context.imageSmoothingQuality = "high";
  context.drawImage(bitmap, x, y, size, size, 0, 0, ICON_SIZE, ICON_SIZE);
  bitmap.close();
  const png = await new Promise<Blob | null>((done) => canvas.toBlob(done, "image/png"));
  if (!png) throw new Error("Couldn't make an icon from that image.");
  return new Uint8Array(await png.arrayBuffer());
}
