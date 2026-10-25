/* Motet: types as documentation. The bundle is framework-neutral: factories return DOM elements
   whose classes are the Panda recipe classes; Svelte and React layers use the recipes directly. */

export type Mode = 'light' | 'dark';
export type Hex = `#${string}`;

/* ---------- theme record (draft lexicon at.motet.theme; all numbers are integers) ---------- */
export interface WorldRecord {
  id?: string; name: string;
  palette: { primary: Hex; secondary?: Hex; neutral?: Hex; scheme?: 'neutral' | 'tonal' | 'vibrant' | 'expressive' | 'fidelity' | 'monochrome';
    /** OKLCH chroma ceiling in thousandths, 0–200 */ chroma?: number; /** −100 cool … 100 warm */ temperature?: number;
    depth?: { light?: -2 | -1 | 0 | 1 | 2; dark?: -2 | -1 | 0 | 1 | 2 }; mode?: Mode; dominant?: Hex;
    overrides?: { light?: Record<string, Hex>; dark?: Record<string, Hex> } };
  material?: { finish?: 'matte' | 'satin' | 'sheen'; /** 0 none, 1 fine, 2 medium, 3 coarse, 4 heavy */ grain?: 0 | 1 | 2 | 3 | 4; edge?: 0 | 1 | 2; shadow?: 0 | 1 | 2; /** 0–12 percent */ tint?: number; /** tint surfaces toward the art's dominant colour */ mica?: boolean;
    /** the content sheet over the art; translucent sheets get a proved-legible opacity and straight edges */ sheet?: 'solid' | 'veil' | 'frosted' };
  shape?: { corner?: 'round' | 'squircle' | 'bevel' | 'square'; /** 0–150 percent */ scale?: number;
    /** framed surfaces only: one chamfer (cut) or two opposite chamfers (clipped) */ profile?: 'even' | 'cut' | 'clipped';
    border?: 'none' | 'hairline' | 'rule' | 'heavy'; /** frame lines in the ornament colour */ tone?: 'neutral' | 'ornament'; frame?: 'none' | 'double' };
  ornament?: { set?: 'rule' | 'fade' | 'filigree' | 'segment'; level?: 0 | 1 | 2 | 3; /** where the art meets the page */ edge?: 'fade' | 'straight' | 'torn' | 'burned' | 'arch'; dropCap?: boolean;
    custom?: { motif?: Ornament; corner?: Ornament; flank?: Ornament } };
  type?: { display?: 'motet' | 'motet-condensed' | 'source-serif' | 'oxanium' | 'big-shoulders-stencil' | 'cormorant' | 'grenze-gotisch' | 'archivo' | 'mono';
    case?: 'none' | 'upper'; /** thousandths of an em */ tracking?: number; weight?: number; names?: 'ui' | 'display';
    reading?: 'serif' | 'sans'; numerals?: 'proportional' | 'tabular' | 'oldstyle'; paragraphs?: 'spaced' | 'indented' };
  imagery?: { art?: string; /** one image: fixed behind the page with a header window (continuous), only around the content (backdrop), or at the top only (header) */ placement?: 'continuous' | 'backdrop' | 'header'; /** 'art' stands the World's title on the art above the edge; 'hidden' leaves it to a wordmark in the art */ title?: 'page' | 'art' | 'hidden'; titleBacking?: 'shade' | 'halo' | 'none'; /** each 0–100, 0 is off; they combine */ treatments?: { veil?: number; vignette?: number; duotone?: number; blur?: number; parallax?: number };
    /** percent; the point shown in the header window */ focus?: [number, number];
    /** measured at publish: darkest and lightest colours of the blurred art */ range?: [Hex, Hex]; dominant?: Hex; /** deprecated: one treatment and its strength, read as treatments: { [treatment]: strength } */ treatment?: 'none' | 'veil' | 'vignette' | 'duotone' | 'blur'; strength?: number };
  icons?: { style?: 'outlined' | 'rounded' | 'sharp'; weight?: number };
  motion?: { character?: 'standard' | 'crisp' | 'soft' | 'eerie' };
  density?: 'comfortable' | 'standard' | 'compact';
  composition?: { post?: 'open' | 'card' | 'dossier' | 'log'; portrait?: 'gutter' | 'inline' | 'none'; names?: 'plain' | 'plate'; plate?: 'ink' | 'accent' };
  chrome?: { /** 0–8 percent */ tint?: number };
}
/** Vector ornament: viewBox size and up to 16 paths (path data only). `s` = stroke width, `m` = mirror, `rule` = fill rule. */
export interface Ornament { vb: [number, number]; p: Array<{ d: string; s?: number; m?: boolean; rule?: 'evenodd' | 'nonzero' }> }

export interface ReaderPrefs {
  plain?: boolean; decoration?: 'on' | 'off'; reduceTransparency?: boolean; reduceMotion?: boolean; contrast?: 'standard' | 'high';
  readingFont?: 'world' | 'serif' | 'sans' | 'system'; textScale?: number; density?: 'comfortable' | 'standard' | 'compact'; coarse?: boolean;
  chromeMaterial?: 'solid' | 'veil' | 'frosted';
}
export interface CompileOptions { mode?: Mode | 'world'; contrast?: 'standard' | 'high'; reader?: ReaderPrefs; supports?: { cornerShape: boolean; backdrop: boolean } }
/** Every place the engine moved a colour away from what the record asked for, and why. */
export interface Adjustment { role: string; kind: 'contrast' | 'chroma' | 'gamut' | 'hue' | 'override'; from: string | null; to: string; target: number | null; text: string }
export interface Compiled { vars: Record<string, string>; attrs: Record<string, string>; palette: Palette; mode: Mode; notes: string[]; adjustments: Adjustment[] }
export interface Palette { mode: Mode; hex: Record<string, string>; notes: string[]; adjustments: Adjustment[] }

/** Motet's own appearance as a theme record. */
export declare const defaultRecord: WorldRecord;
/** The starting themes, by key. */
export declare const presets: { [key: string]: WorldRecord };
export declare function compileWorld(record: WorldRecord, opts?: CompileOptions): Compiled;
export declare function applyWorld(el: HTMLElement, record: WorldRecord, opts?: CompileOptions): Compiled;
export declare function mountWorld(el: HTMLElement, record: WorldRecord, opts?: CompileOptions): { update(record?: WorldRecord, opts?: CompileOptions): Compiled; result: Compiled };
/** Runs a state change as a view transition when the browser can and the reader allows motion; otherwise runs it at once.
 *  `types` ('forward', 'back', 'expand', 'collapse', 'post', 'roll'…) go to the transition and to <html data-vt>. */
export declare function transition(update: () => void | Promise<unknown>, opts?: { types?: string[] }): { finished: Promise<unknown>; ready: Promise<unknown>; updateCallbackDone: Promise<unknown>; skipTransition(): void; skipped?: boolean };
export declare function buildPalette(record: WorldRecord, mode: Mode, opts?: { contrast?: 'standard' | 'high' }): Palette;
export declare function audit(p: Palette): Array<{ role: string; on: string[]; ratio: number; min: number; pass: boolean }>;
export declare function spring(o: { duration: number; bounce?: number; points?: number }): { easing: string; ms: number };
export declare function validateOrnament(o: Ornament): { ok: boolean; errors: string[] };
export declare function toStandardMode(record: WorldRecord, opts?: { mode?: Mode; fonts?: unknown[]; createdAt?: string }): object;
export declare function toDTCG(record: WorldRecord): { light: object; dark: object };
export declare function enhance(root?: ParentNode): void;

/* ---------- component factories ---------- */
export interface ButtonProps { label: string; variant?: 'filled' | 'tonal' | 'outline' | 'text'; tone?: 'accent' | 'danger' | 'neutral'; size?: 'sm' | 'md'; icon?: string; trailingIcon?: string; disabled?: boolean; href?: string; onClick?: (e: Event) => void }
export interface IconButtonProps { icon: string; label: string; variant?: 'standard' | 'tonal' | 'filled'; size?: 'sm' | 'md'; pressed?: boolean | 'true' | 'false'; expanded?: boolean; count?: number; tip?: boolean }
export interface ChipProps { label: string; kind?: 'filter' | 'note' | 'fact'; selected?: boolean; icon?: string; tone?: 'caution' | 'danger' | 'positive' }
export interface AvatarProps { kind?: 'account' | 'character' | 'world'; name: string; src?: string; size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl' | false; hue?: number; decorative?: boolean }
export interface DividerProps {}
export interface PostProps { character?: { name: string; epithet?: string; portrait?: string }; account: { handle: string; display?: string; did?: string }; role?: 'gm' | 'mod' | 'bot';
  lane?: 'ic' | 'ooc'; time?: string; edited?: string; unread?: boolean; /** the Scene event this post answers; the event stays in the stream */ follows?: { label: string; href: string; icon?: string }; notes?: string[]; lead?: boolean; /** sanitised HTML rendered from the canonical Typst body */ body: string }
/** Faces from DiceFont for d2, d4, d6, d8, d10, d12, d20 and Fate ('F'); other dice show numbers. `sides` defaults to the first die in `expr`. */
export interface DiceRollProps { label: string; expr: string; sides?: number | 'F'; results: number[]; dropped?: number[]; total: number; outcome?: string; outcomeIcon?: string; outcomeTone?: 'positive' | 'caution' | 'danger' }
/** One entry between posts: a roll, a roll request, a tool's result or a Scene change. Never part of a post. */
export interface SceneEventProps { kind?: 'roll' | 'request' | 'tool' | 'system'; title: string | Node; expr?: string; icon?: string; time?: string; datetime?: string; timeFull?: string;
  /** who acted: the character and the account; `detail` replaces the generated line */ character?: string; account?: { handle: string; display?: string; did?: string }; detail?: string | Node | Node[];
  roll?: DiceRollProps; /** any other content: a table, a form, a tile frame */ body?: Node | Node[]; source?: { label: string; icon?: string; href?: string }; actions?: Node[]; lane?: 'ic' | 'ooc'; label?: string }
export interface SceneListProps { title: string; location?: string; arc?: string; cast?: string[]; unread?: boolean; unreadCount?: number; turn?: boolean; draft?: boolean; updated?: string; href?: string }
export interface InboxProps { kind: 'turn' | 'mention' | 'application' | 'post' | 'decision' | 'draft'; world: string; worldMark?: string; title: string; excerpt?: string; time?: string; unread?: boolean; resolved?: boolean }
export interface CharacterCardProps { name: string; epithet?: string; portrait?: string; account: string; facts?: Array<[string, string]>; status?: 'approved' | 'pending' }
export interface MastheadProps { name: string; mark?: string; hue?: number; gms: string[]; state?: string; stateIcon?: string; language?: string; size?: 'full' | 'slim'; current?: string; artAlt?: string; nav?: boolean }
export interface ChromeProps { world?: string; worldMark?: string; account: string; inbox?: number }
/** Draft state shown as one icon; its words are the tooltip and accessible name. */
export type DraftSync = 'saved' | 'syncing' | 'synced' | 'local' | 'failed';
export interface ComposerProps { speaker: string; portrait?: string; speakerHue?: number; scene: string; lane?: 'ic' | 'ooc'; audience?: string; audienceIcon?: string; sync?: DraftSync; text?: string; placeholder?: string; submit?: string }
export interface SegmentedProps { label: string; value: string; items: Array<{ value: string; label: string; icon?: string }> }
export interface SwitchProps { label: string; checked?: boolean; id?: string }

export declare function Button(p: ButtonProps): HTMLElement;
export declare function IconButton(p: IconButtonProps): HTMLButtonElement;
export declare function Chip(p: ChipProps): HTMLElement;
export declare function Avatar(p: AvatarProps): HTMLElement;
export declare function Divider(p?: DividerProps): HTMLElement;
export declare function Post(p: PostProps): HTMLElement;
export declare function DiceRoll(p: DiceRollProps & { caption?: boolean }): HTMLElement;
export declare function SceneEvent(p: SceneEventProps): HTMLElement;
export declare function SceneRow(p: SceneListProps): HTMLAnchorElement;
export declare function InboxItem(p: InboxProps): HTMLAnchorElement;
export declare function CharacterCard(p: CharacterCardProps): HTMLElement;
export declare function Masthead(p: MastheadProps): HTMLElement;
export declare function Chrome(p: ChromeProps): HTMLElement;
export declare function Composer(p: ComposerProps): HTMLElement;
export declare function Segmented(p: SegmentedProps): HTMLElement;
export declare function Switch(p: SwitchProps): HTMLButtonElement;
export declare function Tabs(items: Array<[icon: string, label: string]>, current: string): HTMLElement;

/* ---------- kit: forms, disclosure, navigation, overlays, feedback ---------- */
export interface Option { value: string; label: string; description?: string; icon?: string; avatar?: Partial<AvatarProps>; group?: string; keywords?: string; disabled?: boolean }
export interface FieldProps { label?: string; help?: string; error?: string; control: HTMLElement; id?: string }
export interface TextInputProps { label: string; value?: string; type?: string; name?: string; placeholder?: string; autocomplete?: string; inputmode?: string; help?: string; error?: string; leading?: string | HTMLElement; trailing?: string | HTMLElement; actions?: HTMLElement[]; disabled?: boolean; required?: boolean }
export interface CheckboxProps { label: string; description?: string; checked?: boolean; indeterminate?: boolean; disabled?: boolean; name?: string; value?: string; id?: string; onChange?: (checked: boolean) => void }
export interface CheckboxGroupProps { legend: string; help?: string; allLabel?: string; name?: string; items: CheckboxProps[] }
export interface FieldsetProps { legend: string; help?: string; hideLegend?: boolean; children: HTMLElement | HTMLElement[] }
export interface RadioGroupProps { legend: string; help?: string; hideLegend?: boolean; name?: string; value?: string; variant?: 'list' | 'cards'; items: Array<{ value: string; label: string; description?: string; icon?: string; disabled?: boolean }>; onChange?: (value: string) => void }
export interface TextareaProps { label?: string; help?: string; value?: string; name?: string; placeholder?: string; rows?: number; maxRows?: number; maxLength?: number; face?: 'reading' | 'ui' }
export interface PasswordInputProps { label: string; help?: string; name?: string; autocomplete?: 'current-password' | 'new-password' }
export interface NumberInputProps { label: string; help?: string; value?: number; min?: number; max?: number; step?: number; unit?: string; name?: string }
export interface SelectProps { label: string; options: Option[]; value?: string; placeholder?: string; name?: string; help?: string; icon?: string; disabled?: boolean }
export interface ComboboxProps { label: string; options: Option[]; value?: string; placeholder?: string; help?: string; leading?: string; allowCustom?: boolean; empty?: string }
export interface TagsInputProps { label: string; value?: string[]; suggestions?: Option[]; placeholder?: string; max?: number; help?: string; addOnBlur?: boolean }
/** A choice inside a sentence. `label` names it for assistive technology; `all` and `none` add quick options and are the words used when every or no option is picked. Options may carry `short`, the word the sentence uses. */
export interface InlineSelectProps { label: string; options: (Option & { short?: string })[]; value?: string | string[]; multiple?: boolean; all?: string; none?: string; placeholder?: string; groupLabel?: string; maxNames?: number; align?: 'start' | 'end' | 'center'; disabled?: boolean }
export interface CalendarProps { value?: string; min?: string; max?: string; today?: string; locale?: string; weekStart?: 0 | 1 | 2 | 3 | 4 | 5 | 6; isDisabled?: (d: Date) => boolean; footer?: boolean }
export interface DatePickerProps extends CalendarProps { label: string; help?: string; name?: string }
export interface TimeFieldProps { label: string; value?: string; step?: number; help?: string; name?: string; zone?: string; zoneName?: string; locale?: string }
export interface SliderProps { label: string; min?: number; max?: number; step?: number; value?: number; unit?: string; format?: (v: number) => string; marks?: string[]; name?: string; disabled?: boolean }
export interface UploadApi { add(f: { name: string; size: number; type?: string; url?: string; state?: 'ready' | 'uploading' | 'error'; progress?: number }): string; setState(id: string, state: 'ready' | 'uploading' | 'error', message?: string | null, progress?: number): void }
export interface FileUploadProps { label?: string; accept?: string; maxBytes?: number; multiple?: boolean; title?: string; hint?: string }
export interface AccordionProps { items: Array<{ title: string; meta?: string; icon?: string; content: string | HTMLElement; open?: boolean }>; multiple?: boolean; variant?: 'flush' | 'framed' }
export interface CollapsibleProps { label?: string; content: string | HTMLElement; open?: boolean; variant?: 'default' | 'spoiler'; topic?: string }
export interface TreeItem { id?: string; label: string; icon?: string; count?: number; children?: TreeItem[]; expanded?: boolean; selected?: boolean }
export interface PaginationProps { page: number; count: number; href?: (page: number) => string; label?: string }
export interface BreadcrumbProps { items: Array<{ label: string; href?: string; icon?: string }>; max?: number; label?: string }
export interface StepsProps { items: Array<{ title: string; description?: string }>; current: number; orientation?: 'auto' | 'vertical'; label?: string }
export interface PopoverProps { trigger: HTMLElement; title?: string; label?: string; content: HTMLElement | HTMLElement[]; footer?: HTMLElement[]; width?: number; align?: 'start' | 'center' | 'end' }
export interface HoverCardProps { trigger: HTMLElement; content: HTMLElement; delay?: number }
export interface CharacterPreviewProps { name: string; epithet?: string; portrait?: string; hue?: number; facts?: Array<[string, string]>; account?: string }
export interface DrawerProps { title: string; content: HTMLElement; footer?: HTMLElement[]; parent?: HTMLElement }
export interface ToastOptions { message: string; tone?: 'neutral' | 'info' | 'positive' | 'caution' | 'danger'; icon?: string; action?: { label: string; onClick?: () => void }; duration?: number }
export interface AlertProps { tone?: 'info' | 'caution' | 'danger' | 'positive' | 'neutral'; title?: string; description?: string | HTMLElement; actions?: HTMLElement[]; onClose?: (el: HTMLElement) => void; variant?: 'inline' | 'banner'; icon?: string; live?: boolean }
export interface ProgressProps { label: string; value?: number; max?: number; valueText?: string; tone?: 'danger' }
export interface ProgressRingProps { value?: number; max?: number; size?: number; stroke?: number; showValue?: boolean; label: string }
export interface DataListProps { items: Array<[string, string | HTMLElement]>; variant?: 'plain' | 'ruled' | 'stacked'; labelWidth?: string }

export declare function Field(p: FieldProps): HTMLElement;
export declare function TextInput(p: TextInputProps): HTMLElement;
export declare function Checkbox(p: CheckboxProps): HTMLLabelElement;
export declare function CheckboxGroup(p: CheckboxGroupProps): HTMLFieldSetElement;
export declare function Fieldset(p: FieldsetProps): HTMLFieldSetElement;
export declare function RadioGroup(p: RadioGroupProps): HTMLFieldSetElement;
export declare function Textarea(p: TextareaProps): HTMLElement;
export declare function PasswordInput(p: PasswordInputProps): HTMLElement;
export declare function NumberInput(p: NumberInputProps): HTMLElement;
/** emits `change` with { value, option } */
export declare function Select(p: SelectProps): HTMLElement & { value?: string; open(): void; close(focus?: boolean): void };
export declare function Combobox(p: ComboboxProps): HTMLElement & { value: string | null; open(query?: string): void };
export declare function TagsInput(p: TagsInputProps): HTMLElement & { open(query?: string): void };
export declare function InlineSelect(p: InlineSelectProps): HTMLElement & { value: string | string[] | null; open(): void; close(focus?: boolean): void; setValue(v: string | string[] | null): void };
export declare function Calendar(p: CalendarProps): HTMLElement & { value?: string; focusGrid(): void; setValue(v: string): void };
export declare function DatePicker(p: DatePickerProps): HTMLElement & { value: string | null; open(): void };
export declare function TimeField(p: TimeFieldProps): HTMLElement;
export declare function Slider(p: SliderProps): HTMLElement & { value: number };
export interface ColorGround { label: string; hex: string; min?: number; sample?: string }
export interface ColorPickerProps { label?: string; value?: string | null; help?: string; placeholder?: string; clearable?: boolean; inline?: boolean; swatches?: (string | { value: string; label?: string })[]; swatchesLabel?: string; against?: ColorGround[]; footer?: HTMLElement; hue?: number }
/** OKLCH picker. Emits `input` and `change` with `{ value: '#rrggbb' | null, lch: { l, c, h } }`. */
export declare function ColorPicker(p: ColorPickerProps): HTMLElement & { value: string | null; open(): void; close(focus?: boolean): void; setValue(hex: string | null): void; setAgainst(list: ColorGround[]): void; setHelp(text: string, tone?: 'danger' | 'caution'): void };
/** emits `files` with { id, file, api } */
export declare function FileUpload(p: FileUploadProps): HTMLElement & { api: UploadApi };
export declare function Accordion(p: AccordionProps): HTMLElement;
export declare function Collapsible(p: CollapsibleProps): HTMLElement;
/** emits `select` with { id, item } and `toggle` */
export declare function TreeView(p: { label: string; items: TreeItem[] }): HTMLUListElement;
export declare function Pagination(p: PaginationProps): HTMLElement;
export declare function Breadcrumb(p: BreadcrumbProps): HTMLElement;
export declare function Steps(p: StepsProps): HTMLElement;
export declare function Popover(p: PopoverProps): HTMLElement & { open(): void; close(focus?: boolean): void };
export declare function HoverCard(p: HoverCardProps): HTMLElement;
export declare function CharacterPreview(p: CharacterPreviewProps): HTMLElement;
export declare function Drawer(p: DrawerProps): HTMLDialogElement & { openModal(): void };
export declare function toast(o: ToastOptions): { el: HTMLElement; dismiss(): void };
export declare function Alert(p: AlertProps): HTMLElement;
export declare function Progress(p: ProgressProps): HTMLElement;
export declare function ProgressRing(p: ProgressRingProps): HTMLElement;
export declare function Spinner(p?: { size?: number; label?: string }): HTMLElement;
export declare function Skeleton(p?: { shape?: 'line' | 'block' | 'circle'; width?: string; height?: string }): HTMLElement;
export declare function PostSkeleton(): HTMLElement;
export declare function DataList(p: DataListProps): HTMLDListElement;
/** Positioning for custom popups: top-layer popover, flipped and clamped to the viewport. */
export declare const popup: { open(el: HTMLElement, anchor: HTMLElement, o?: { align?: 'start' | 'center' | 'end'; matchWidth?: boolean; offset?: number }): void; close(el: HTMLElement): void };

declare global { interface Window { Motet: typeof import('./motet.js') } }

export type Lane = 'ic' | 'ooc' | 'both';
/** The lane after a click; `policy` decides a click while both are on ('unselect' hides the clicked lane, 'solo' keeps only it). */
export declare function nextLane(current: Lane, clicked: 'ic' | 'ooc', policy?: 'unselect' | 'solo'): Lane;
export declare const lanePolicy: { fromBoth: 'unselect' | 'solo' };
export interface LaneFilterProps { value?: Lane; policy?: 'unselect' | 'solo'; labels?: Partial<Record<'group' | 'ic' | 'ooc' | 'both' | 'icOnly' | 'oocOnly', string>>; class?: string }
/** Two toggles in one split button, one always on. Emits `change` with `{ value: Lane }`. */
export declare function LaneFilter(p?: LaneFilterProps): HTMLElement & { value: Lane; setValue(v: Lane): void };
export declare const treatments: Array<"veil" | "vignette" | "duotone" | "blur" | "parallax">;
export declare function treatmentsOf(imagery?: WorldRecord["imagery"]): Record<"veil" | "vignette" | "duotone" | "blur" | "parallax", number>;
