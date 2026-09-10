/**
 * The HTML side of the converter: escaping, what CSS may be handed, and the
 * slots that carry finished HTML through the escaping.
 */

export const escapeHtml = (text: string) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** A colour the charte gave, in the one shape CSS accepts unquestioned. */
export const SAFE_COLOUR = /^#[0-9a-fA-F]{3,8}$/;

/** Lengths TeX and CSS agree on, so a rule or a gap can be drawn as-is. */
export const SAFE_LENGTH = /^-?\d*\.?\d+(pt|mm|cm|in|em|ex|px)$/;

/**
 * Parking for HTML that must survive the escaping of the prose around it.
 *
 * Every fragment of HTML the converter produces early — maths, figures — is
 * parked here BEFORE the text is escaped, and restored at the very end. It was
 * injected in place once, and the escaping showed it to the teacher as literal
 * markup.
 */
export class Slots {
  private readonly parked: string[] = [];

  /** Hands back the marker that stands in for `html` until `restore`. */
  park(html: string): string {
    this.parked.push(html);
    return `@@PLUME_${this.parked.length - 1}@@`;
  }

  restore(html: string): string {
    return html.replace(/@@PLUME_(\d+)@@/g, (_, index) => this.parked[Number(index)] ?? "");
  }
}
