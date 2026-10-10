import { test, expect, type Page } from "./fixtures/test";
import {
  clickPlay,
  freshCredentials,
  graphql,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 088 US2: a new world can open on one of our maps, and the map is
 * credited wherever it is shown.
 *
 * # Which default this stack has
 *
 * The harness offers the maps but makes none the default
 * (`scripts/e2e/base-maps.mjs`), so the hundreds of worlds the other specs
 * make keep the blank Starting Scene they were written against. So the form
 * here preselects **None**, and the map is chosen with one click before
 * Create. That the server's own default is `grassy-path-ambush`, and that a
 * world asked for without a map gets it, is proven against the real schema
 * in `base_maps/create_world_tests.rs`.
 */

const CREDIT = {
  licenceUrl: "https://creativecommons.org/licenses/by-sa/4.0/",
  source: "https://github.com/mbround18/vtt-maps",
  catalog: "https://vtt-maps.dnd-apps.dev/catalog",
};

const SCENES = `
  query BaseMapScenes($worldId: UUID!) {
    scenes(worldId: $worldId) {
      sceneId worldId name description type gridSize gridType width height
      backgroundImagePath backgroundUrl backgroundGridMismatch ownerId
      createdAt updatedAt summaryMarkdown summaryRenderedHtml hidden
      previewUrl ambientLight
      backgroundCredit { author licence licenceUrl source catalog shareAlike }
    }
  }
`;

const WALLS = `
  query BaseMapWalls($sceneId: UUID!) {
    walls(sceneId: $sceneId) { wallId }
    lightSources(sceneId: $sceneId) { lightId }
  }
`;

const BASE_MAPS = `
  query BaseMapSizes { baseMaps { id width height gridSize } }
`;

const CREATE_WORLD = `
  mutation BaseMapTodaysWorld($input: GraphQLCreateWorldInput!) {
    createWorld(input: $input) { id activeSceneId }
  }
`;

type Scene = Record<string, unknown> & { sceneId: string };

async function startingScene(page: Page, worldId: string): Promise<Scene> {
  const result = await graphql<{ data?: { scenes?: Scene[] } }>(page, SCENES, {
    worldId,
  });
  const scenes = result.data?.scenes ?? [];
  expect(scenes, "a new world has exactly its Starting Scene").toHaveLength(1);
  return scenes[0];
}

async function contents(page: Page, sceneId: string) {
  const result = await graphql<{
    data?: { walls?: unknown[]; lightSources?: unknown[] };
  }>(page, WALLS, { sceneId });
  return {
    walls: result.data?.walls?.length ?? -1,
    lights: result.data?.lightSources?.length ?? -1,
  };
}

/** The create form, filled with a name, with the picker on screen. */
async function openCreateForm(page: Page, name: string): Promise<void> {
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(name);
  await expect(page.getByTestId("base-map-picker")).toBeVisible({
    timeout: 15_000,
  });
}

async function create(page: Page): Promise<string> {
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 30_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) throw new Error(`no world id in ${page.url()}`);
  return match[1];
}

test.describe("A new world opens on one of our maps, credited", () => {
  test("a chosen map is on the board with its credit, with no step after Create", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2ebasemap"));
    await openCreateForm(page, `Base Map World ${uniqueSuffix()}`);

    // This stack has no default, so the form starts on None (see the top).
    await expect(
      page.getByTestId("base-map-option-none").getByRole("radio"),
    ).toBeChecked();
    // Credited once, beside the choice.
    await expect(
      page.getByTestId("base-map-picker").getByTestId("map-credit"),
    ).toContainText("MBRound18");

    await page.getByTestId("base-map-option-grassy-path-ambush").click();
    await expect(
      page.getByTestId("base-map-option-grassy-path-ambush").getByRole("radio"),
    ).toBeChecked();
    const worldId = await create(page);

    const scene = await startingScene(page, worldId);
    expect(scene.backgroundUrl, "the map is the scene's background").toEqual(
      expect.any(String),
    );
    expect(scene.backgroundCredit).toMatchObject({
      author: "MBRound18",
      licence: "CC BY-SA 4.0",
      ...CREDIT,
    });
    // The scene takes the map's own size and grid. The map is an outdoor one
    // with no walls of its own; its edge walls are proven with US6.
    const listed = await graphql<{
      data?: {
        baseMaps?: {
          id: string;
          width: number;
          height: number;
          gridSize: number;
        }[];
      };
    }>(page, BASE_MAPS);
    const map = listed.data?.baseMaps?.find(
      (m) => m.id === "grassy-path-ambush",
    );
    expect(map, JSON.stringify(listed)).toBeTruthy();
    expect({
      width: scene.width,
      height: scene.height,
      gridSize: scene.gridSize,
    }).toEqual({
      width: map!.width,
      height: map!.height,
      gridSize: map!.gridSize,
    });

    await clickPlay(page);
    const line = page.getByTestId("board-map-credit");
    await expect(line).toBeVisible({ timeout: 30_000 });
    await expect(line).toContainText("Map: MBRound18, CC BY-SA 4.0");

    // The scene list and the scene's own page credit it as well.
    await page.goto(`/world/${worldId}/scenes`);
    await expect(
      page.getByTestId(`scene-row-${scene.sceneId}`).getByTestId("map-credit"),
    ).toContainText("MBRound18");
    await page.goto(`/world/${worldId}/scenes/${scene.sceneId}`);
    await expect(
      page.getByTestId("scene-preview-card").getByTestId("map-credit"),
    ).toContainText("CC BY-SA 4.0");
  });

  test("None makes the Starting Scene a world gets today, field for field", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2ebasenone"));

    // Today's: a world made the way every client made one before spec 088,
    // naming no map at all.
    const todays = await graphql<{
      data?: { createWorld?: { id: string } };
    }>(page, CREATE_WORLD, {
      input: { name: `Todays World ${uniqueSuffix()}` },
    });
    const todaysId = todays.data?.createWorld?.id;
    expect(todaysId, JSON.stringify(todays)).toBeTruthy();

    await openCreateForm(page, `None World ${uniqueSuffix()}`);
    await page.getByTestId("base-map-option-none").click();
    const worldId = await create(page);

    const made = await startingScene(page, worldId);
    const baseline = await startingScene(page, todaysId!);
    // Everything but what makes two scenes two scenes.
    const comparable = (scene: Scene) =>
      Object.fromEntries(
        Object.entries(scene).filter(
          ([key]) =>
            !["sceneId", "worldId", "createdAt", "updatedAt"].includes(key),
        ),
      );
    expect(comparable(made)).toEqual(comparable(baseline));
    expect(made.backgroundUrl).toBeNull();
    expect(made.backgroundCredit).toBeNull();
    expect(await contents(page, made.sceneId)).toEqual(
      await contents(page, baseline.sceneId),
    );
  });

  test("the board's credit opens into links to the licence, the source and the catalog", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2ebaselinks"));
    await openCreateForm(page, `Credit World ${uniqueSuffix()}`);
    await page.getByTestId("base-map-option-grassy-path-ambush").click();
    await create(page);
    await clickPlay(page);

    const line = page.getByTestId("board-map-credit");
    await expect(line).toBeVisible({ timeout: 30_000 });
    await expect(line).toHaveAttribute("data-open", "false");

    // By keyboard: focus opens it, and the links inside keep it open.
    await page.getByTestId("board-map-credit-short").focus();
    await expect(line).toHaveAttribute("data-open", "true");
    await expect(line.getByTestId("map-credit-licence")).toHaveAttribute(
      "href",
      CREDIT.licenceUrl,
    );
    await expect(line.getByTestId("map-credit-source")).toHaveAttribute(
      "href",
      CREDIT.source,
    );
    await expect(line.getByTestId("map-credit-catalog")).toHaveAttribute(
      "href",
      CREDIT.catalog,
    );
    await expect(line).toContainText(
      "The copies here are offered under the same licence.",
    );
    await page.keyboard.press("Tab");
    await expect(line).toHaveAttribute("data-open", "true");
  });
});
