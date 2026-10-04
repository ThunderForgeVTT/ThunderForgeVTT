/**
 * What the people running this instance wrote for their own terms.
 *
 * Setup asks an operator who publishes for three pieces of prose — who may
 * join, what this community adds to the shipped rules, and how a change to the
 * terms is announced — and stored all three. Nothing rendered them: an
 * operator filled the step in, opened their terms page, and found none of it.
 *
 * # Why it is its own cards rather than text inside the document
 *
 * `legal/terms-of-service.md` is a versioned document people attest to. Its
 * text is the same on every instance and is compiled in; splicing an
 * operator's paragraphs into the middle of it would make the attested text
 * differ per instance while its version did not. So the operator's words sit
 * beside the document, under a heading that says whose they are.
 *
 * # Why this is plain text
 *
 * For the reason `LegalProse` gives for operator tokens: this is text an
 * operator typed, not text this repository authors, and it is never handed to
 * a parser. A blank line starts a new paragraph and that is the whole of the
 * formatting — a `[link](…)` an operator writes renders as those characters.
 *
 * # Why an unwritten block renders nothing
 *
 * FR-030: what an operator did not answer is not composed for them. There is
 * no marker and no stand-in here — a block with no text has no card, and an
 * operator who wrote none of the three adds nothing to the page.
 */
import { Card } from "@/components/ui/card/Card";
import {
  usePublishedOperatorProse,
  type OperatorProse,
} from "@/legal/publishedOperatorValues";

interface Block {
  key: keyof OperatorProse;
  testId: string;
  heading: string;
}

const BLOCKS: readonly Block[] = [
  {
    key: "minimumAgeStatement",
    testId: "legal-operator-prose-minimum-age-statement",
    heading: "Who may join this instance",
  },
  {
    key: "communityAddendum",
    testId: "legal-operator-prose-community-addendum",
    heading: "This community's own rules",
  },
  {
    key: "termsChangeNotice",
    testId: "legal-operator-prose-terms-change-notice",
    heading: "How you will hear about changes",
  },
];

function paragraphs(text: string): string[] {
  return text
    .split(/\n\s*\n/)
    .map((paragraph) => paragraph.trim())
    .filter((paragraph) => paragraph.length > 0);
}

export function OperatorTermsProse() {
  const prose = usePublishedOperatorProse();
  const written = BLOCKS.filter((block) => prose[block.key].length > 0);

  if (written.length === 0) {
    return null;
  }

  return (
    <>
      {written.map((block) => (
        <Card
          key={block.key}
          surface="parchment"
          className="grid gap-3 p-6"
          data-testid={block.testId}
        >
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            From the people who run this instance
          </p>
          <h2 className="text-lg font-semibold">{block.heading}</h2>
          <div className="grid gap-3 text-sm leading-relaxed">
            {paragraphs(prose[block.key]).map((paragraph, index) => (
              <p key={index} className="whitespace-pre-line">
                {paragraph}
              </p>
            ))}
          </div>
        </Card>
      ))}
    </>
  );
}
