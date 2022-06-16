import { readFile } from "fs/promises";
import { sortBy } from "lodash-es";
import path from "path";
import React from "react";
import { Trans, useTranslation } from "react-i18next";
import semver from "semver";

import { app, shell } from "@electron/remote";
import FolderOpenIcon from "@mui/icons-material/FolderOpen";
import KeyboardArrowDownIcon from "@mui/icons-material/KeyboardArrowDown";
import KeyboardArrowUpIcon from "@mui/icons-material/KeyboardArrowUp";
import RefreshIcon from "@mui/icons-material/Refresh";
import SportsEsportsOutlinedIcon from "@mui/icons-material/SportsEsportsOutlined";
import WarningIcon from "@mui/icons-material/Warning";
import Box from "@mui/material/Box";
import Collapse from "@mui/material/Collapse";
import FormControl from "@mui/material/FormControl";
import IconButton from "@mui/material/IconButton";
import InputLabel from "@mui/material/InputLabel";
import ListItemText from "@mui/material/ListItemText";
import ListSubheader from "@mui/material/ListSubheader";
import MenuItem from "@mui/material/MenuItem";
import Select from "@mui/material/Select";
import Stack from "@mui/material/Stack";
import Tooltip from "@mui/material/Tooltip";
import Typography from "@mui/material/Typography";

import { getBasePath, getSavesPath } from "../../../paths";
import { SetSettings } from "../../../protos/generated/lobby";
import { FAMILY_BY_ROM_NAME, KNOWN_ROM_FAMILIES } from "../../../rom";
import { Editor, editorClassForGameFamily } from "../../../saveedit";
import { fallbackLng } from "../../i18n";
import BattleStarter, { useGetNetplayCompatibility } from "../BattleStarter";
import { usePatches } from "../PatchesContext";
import { useROMs } from "../ROMsContext";
import { useSaves } from "../SavesContext";
import SaveViewer from "../SaveViewer";

function SaveViewerWrapper({
  filename,
  romName,
  incarnation,
}: {
  filename: string;
  romName: string;
  incarnation: number;
}) {
  const [editor, setEditor] = React.useState<Editor | null>(null);

  React.useEffect(() => {
    (async () => {
      const Editor = editorClassForGameFamily(FAMILY_BY_ROM_NAME[romName]);
      setEditor(
        new Editor(
          Editor.sramDumpToRaw(
            (await readFile(path.join(getSavesPath(app), filename))).buffer
          ),
          romName,
          false
        )
      );
    })();
  }, [filename, romName, incarnation]);

  if (editor == null) {
    return null;
  }

  return <SaveViewer editor={editor} />;
}

export default function SavesPane({ active }: { active: boolean }) {
  const { saves, rescan: rescanSaves } = useSaves();
  const { patches, rescan: rescanPatches } = usePatches();
  const { roms, rescan: rescanROMs } = useROMs();
  const { i18n } = useTranslation();

  const [patchOptionsOpen, setPatchOptionsOpen] = React.useState(false);
  const [battleReady, setBattleReady] = React.useState(false);

  const [selectedSave_, setSelectedSave] = React.useState<{
    romName: string;
    saveName: string;
  } | null>(null);
  const [incarnation, setIncarnation] = React.useState(0);
  const [opponentSettings, setOpponentSettings] =
    React.useState<SetSettings | null>(null);
  const opponentAvailableGames = opponentSettings?.availableGames ?? [];

  const getNetplayCompatibility = useGetNetplayCompatibility();

  const selectedSave =
    selectedSave_ != null &&
    Object.prototype.hasOwnProperty.call(saves, selectedSave_.saveName) &&
    Object.prototype.hasOwnProperty.call(roms, selectedSave_.romName)
      ? selectedSave_
      : null;

  const groupedSaves: { [key: string]: string[] } = {};
  for (const k of Object.keys(saves)) {
    for (const romName of saves[k]) {
      groupedSaves[romName] = groupedSaves[romName] || [];
      groupedSaves[romName].push(k);
    }
  }

  const romNames = sortBy(
    Object.values(KNOWN_ROM_FAMILIES).flatMap((f) => Object.keys(f.versions)),
    (k) => {
      const family = KNOWN_ROM_FAMILIES[FAMILY_BY_ROM_NAME[k]];
      const romInfo = family.versions[k];
      return [
        family.title[i18n.resolvedLanguage] || family.title[fallbackLng],
        romInfo.title[i18n.resolvedLanguage] || romInfo.title[fallbackLng],
      ];
    }
  );

  const [patchName_, setPatchName] = React.useState<string | null>(null);
  const patchName =
    patchName_ != null &&
    Object.prototype.hasOwnProperty.call(patches, patchName_)
      ? patchName_
      : null;

  const eligiblePatchNames = React.useMemo(() => {
    const eligiblePatchNames =
      selectedSave != null
        ? Object.keys(patches).filter((p) =>
            Object.values(patches[p].versions).some((v) =>
              v.forROMs.some((r) => r.name == selectedSave.romName)
            )
          )
        : [];
    eligiblePatchNames.sort();
    return eligiblePatchNames;
  }, [patches, selectedSave]);

  const patchInfo = patchName != null ? patches[patchName] : null;

  const patchVersions = React.useMemo(
    () =>
      patchInfo != null ? semver.rsort(Object.keys(patchInfo.versions)) : null,
    [patchInfo]
  );

  const [patchVersion_, setPatchVersion] = React.useState<string | null>(null);
  const patchVersion =
    patchName != null &&
    patchVersion_ != null &&
    Object.prototype.hasOwnProperty.call(
      patches[patchName].versions,
      patchVersion_
    )
      ? patchVersion_
      : null;

  React.useEffect(() => {
    if (patchVersions == null) {
      setPatchVersion(null);
      return;
    }
    setPatchVersion(patchVersions[0]);
  }, [patchVersions]);

  const listFormatter = new Intl.ListFormat(i18n.resolvedLanguage, {
    style: "long",
    type: "conjunction",
  });

  return (
    <Box
      sx={{
        my: 1,
        flexGrow: 1,
        display: active ? "flex" : "none",
      }}
    >
      <Stack sx={{ flexGrow: 1, width: 0 }}>
        <Box flexGrow={0} flexShrink={0} sx={{ px: 1 }}>
          <Stack spacing={1} flexGrow={0} flexShrink={0} direction="row">
            <Tooltip title={<Trans i18nKey="play:show-hide-extra-options" />}>
              <IconButton
                onClick={() => {
                  setPatchOptionsOpen((o) => !o);
                }}
              >
                {patchOptionsOpen ? (
                  <KeyboardArrowUpIcon />
                ) : (
                  <KeyboardArrowDownIcon />
                )}
              </IconButton>
            </Tooltip>
            <FormControl fullWidth size="small">
              <InputLabel id="select-save-label">
                <Trans i18nKey="play:select-save" />
              </InputLabel>
              <Select
                labelId="select-save-label"
                label={<Trans i18nKey="play:select-save" />}
                value={selectedSave != null ? JSON.stringify(selectedSave) : ""}
                disabled={battleReady}
                renderValue={(v) => {
                  if (v == "") {
                    return null;
                  }

                  const selection = JSON.parse(v);

                  return (
                    <>
                      {/* {opponentSettings?.gameInfo != null &&
                      !Object.keys(patches)
                        .filter((p) => patches[p].forROM == selection.romName)
                        .flatMap((p) =>
                          Object.values(patches[p].versions).map(
                            (v) => v.netplayCompatibility
                          )
                        )
                        .concat([FAMILY_BY_ROM_NAME[selection.romName]])
                        .some(
                          (nc) =>
                            nc ==
                            getNetplayCompatibility(opponentSettings!.gameInfo!)
                        ) ? (
                        <Tooltip
                          title={<Trans i18nKey="play:incompatible-game" />}
                        >
                          <WarningIcon
                            color="warning"
                            sx={{
                              fontSize: "1em",
                              marginRight: "8px",
                              verticalAlign: "middle",
                            }}
                          />
                        </Tooltip>
                      ) : opponentAvailableGames.length > 0 &&
                        !opponentAvailableGames.some(
                          (g) => g.rom == selection.romName
                        ) ? (
                        <Tooltip
                          title={<Trans i18nKey="play:no-remote-copy" />}
                        >
                          <WarningIcon
                            color="warning"
                            sx={{
                              fontSize: "1em",
                              marginRight: "8px",
                              verticalAlign: "middle",
                            }}
                          />
                        </Tooltip>
                      ) : null} */}
                      {selection.saveName}{" "}
                      <small>
                        <Trans
                          i18nKey="play:rom-name"
                          values={{
                            familyName:
                              KNOWN_ROM_FAMILIES[
                                FAMILY_BY_ROM_NAME[selection.romName]
                              ].title[i18n.resolvedLanguage] ||
                              KNOWN_ROM_FAMILIES[
                                FAMILY_BY_ROM_NAME[selection.romName]
                              ].title[fallbackLng],
                            versionName:
                              KNOWN_ROM_FAMILIES[
                                FAMILY_BY_ROM_NAME[selection.romName]
                              ].versions[selection.romName].title[
                                i18n.resolvedLanguage
                              ] ||
                              KNOWN_ROM_FAMILIES[
                                FAMILY_BY_ROM_NAME[selection.romName]
                              ].versions[selection.romName].title[fallbackLng],
                          }}
                        />
                      </small>
                    </>
                  );
                }}
                onChange={(e) => {
                  const v = JSON.parse(e.target.value);
                  if (
                    selectedSave == null ||
                    v.romName != selectedSave.romName
                  ) {
                    setPatchName(null);
                    setPatchVersion(null);
                  }
                  setSelectedSave(v);
                }}
              >
                {romNames.flatMap((romName) => {
                  const saveNames = groupedSaves[romName] || [];
                  if (saveNames.length == 0) {
                    return [];
                  }

                  saveNames.sort();

                  return [
                    [
                      <ListSubheader key="title" sx={{ userSelect: "none" }}>
                        <Trans
                          i18nKey="play:rom-name"
                          values={{
                            familyName:
                              KNOWN_ROM_FAMILIES[FAMILY_BY_ROM_NAME[romName]]
                                .title[i18n.resolvedLanguage] ||
                              KNOWN_ROM_FAMILIES[FAMILY_BY_ROM_NAME[romName]]
                                .title[fallbackLng],
                            versionName:
                              KNOWN_ROM_FAMILIES[FAMILY_BY_ROM_NAME[romName]]
                                .versions[romName].title[
                                i18n.resolvedLanguage
                              ] ||
                              KNOWN_ROM_FAMILIES[FAMILY_BY_ROM_NAME[romName]]
                                .versions[romName].title[fallbackLng],
                          }}
                        />
                      </ListSubheader>,
                      ...saveNames.map((v) => {
                        const value = JSON.stringify({ romName, saveName: v });
                        return (
                          <MenuItem
                            key={value}
                            value={value}
                            disabled={
                              !Object.prototype.hasOwnProperty.call(
                                roms,
                                romName
                              )
                            }
                          >
                            {/* {opponentSettings?.gameInfo != null &&
                            !Object.keys(patches)
                              .filter((p) => patches[p].forROM == romName)
                              .flatMap((p) =>
                                Object.keys(patches[p].versions).map(
                                  (v) =>
                                    patches[p].versions[v].netplayCompatibility
                                )
                              )
                              .concat([
                                KNOWN_ROMS[romName].netplayCompatibility,
                              ])
                              .some(
                                (nc) =>
                                  nc ==
                                  getNetplayCompatibility(
                                    opponentSettings!.gameInfo!
                                  )
                              ) ? (
                              <Tooltip
                                title={
                                  <Trans i18nKey="play:incompatible-game" />
                                }
                              >
                                <WarningIcon
                                  color="warning"
                                  sx={{
                                    fontSize: "1em",
                                    marginRight: "8px",
                                    verticalAlign: "middle",
                                  }}
                                />
                              </Tooltip>
                            ) : opponentAvailableGames.length > 0 &&
                              !opponentAvailableGames.some(
                                (g) => g.rom == romName
                              ) ? (
                              <Tooltip
                                title={<Trans i18nKey="play:no-remote-copy" />}
                              >
                                <WarningIcon
                                  color="warning"
                                  sx={{
                                    fontSize: "1em",
                                    marginRight: "8px",
                                    verticalAlign: "middle",
                                  }}
                                />
                              </Tooltip>
                            ) : null}{" "} */}
                            {v}
                          </MenuItem>
                        );
                      }),
                    ],
                  ];
                })}
              </Select>
            </FormControl>
            <Tooltip title={<Trans i18nKey="play:open-dir" />}>
              <IconButton
                onClick={() => {
                  if (selectedSave == null) {
                    shell.openPath(getBasePath(app));
                  } else {
                    shell.showItemInFolder(
                      path.join(getSavesPath(app), selectedSave.saveName)
                    );
                  }
                }}
              >
                <FolderOpenIcon />
              </IconButton>
            </Tooltip>
            <Tooltip title={<Trans i18nKey="play:rescan" />}>
              <IconButton
                onClick={() => {
                  (async () => {
                    await Promise.allSettled([
                      rescanROMs(),
                      rescanPatches(),
                      rescanSaves(),
                    ]);
                  })();
                }}
              >
                <RefreshIcon />
              </IconButton>
            </Tooltip>
          </Stack>
          <Collapse in={patchOptionsOpen}>
            <Stack
              flexGrow={0}
              flexShrink={0}
              justifyContent="flex-end"
              direction="row"
              spacing={1}
              sx={{ px: 1, mt: 1 }}
            >
              <FormControl fullWidth size="small" sx={{ width: "250%" }}>
                <InputLabel id="game-label">
                  <Trans i18nKey="play:patch-name" />
                </InputLabel>
                <Select
                  labelId="game-label"
                  disabled={selectedSave == null || battleReady}
                  size="small"
                  value={JSON.stringify(patchName)}
                  label={<Trans i18nKey="play:patch-name" />}
                  onChange={(e) => {
                    setPatchName(JSON.parse(e.target.value));
                    setPatchVersion(null);
                  }}
                  renderValue={(v) => {
                    const patchName = JSON.parse(v);
                    if (patchName == null) {
                      return (
                        <>
                          {opponentSettings?.gameInfo != null &&
                          selectedSave != null &&
                          FAMILY_BY_ROM_NAME[selectedSave.romName] !=
                            getNetplayCompatibility(
                              opponentSettings.gameInfo
                            ) ? (
                            <Tooltip
                              title={<Trans i18nKey="play:incompatible-game" />}
                            >
                              <WarningIcon
                                color="warning"
                                sx={{
                                  fontSize: "1em",
                                  marginRight: "8px",
                                  verticalAlign: "middle",
                                }}
                              />
                            </Tooltip>
                          ) : opponentAvailableGames.length > 0 &&
                            !opponentAvailableGames.some(
                              (g) =>
                                selectedSave != null &&
                                g.rom == selectedSave.romName &&
                                g.patch == null
                            ) ? (
                            <Tooltip
                              title={<Trans i18nKey="play:no-remote-copy" />}
                            >
                              <WarningIcon
                                color="warning"
                                sx={{
                                  fontSize: "1em",
                                  marginRight: "8px",
                                  verticalAlign: "middle",
                                }}
                              />
                            </Tooltip>
                          ) : null}{" "}
                          <Trans i18nKey="play:unpatched" />
                        </>
                      );
                    }
                    return (
                      <>
                        {opponentSettings?.gameInfo != null &&
                        selectedSave != null &&
                        !Object.keys(patches[patchName].versions)
                          .map(
                            (v) =>
                              patches[patchName].versions[v]
                                .netplayCompatibility
                          )
                          .some(
                            (nc) =>
                              nc ==
                              getNetplayCompatibility(
                                opponentSettings!.gameInfo!
                              )
                          ) ? (
                          <Tooltip
                            title={<Trans i18nKey="play:incompatible-game" />}
                          >
                            <WarningIcon
                              color="warning"
                              sx={{
                                fontSize: "1em",
                                marginRight: "8px",
                                verticalAlign: "middle",
                              }}
                            />
                          </Tooltip>
                        ) : opponentAvailableGames.length > 0 &&
                          !opponentAvailableGames.some(
                            (g) =>
                              selectedSave != null &&
                              g.rom == selectedSave.romName &&
                              g.patch != null &&
                              g.patch.name == patchName
                          ) ? (
                          <Tooltip
                            title={<Trans i18nKey="play:no-remote-copy" />}
                          >
                            <WarningIcon
                              color="warning"
                              sx={{
                                fontSize: "1em",
                                marginRight: "8px",
                                verticalAlign: "middle",
                              }}
                            />
                          </Tooltip>
                        ) : null}{" "}
                        {patches[patchName].title}{" "}
                        <small>
                          <Trans
                            i18nKey="play:patch-byline"
                            values={{
                              authors: listFormatter.format(
                                patches[patchName].authors.flatMap(({ name }) =>
                                  name != null ? [name] : []
                                )
                              ),
                            }}
                          />
                        </small>
                      </>
                    );
                  }}
                  fullWidth
                >
                  <MenuItem value="null">
                    {opponentSettings?.gameInfo != null &&
                    selectedSave != null &&
                    FAMILY_BY_ROM_NAME[selectedSave.romName] !=
                      getNetplayCompatibility(opponentSettings.gameInfo) ? (
                      <Tooltip
                        title={<Trans i18nKey="play:incompatible-game" />}
                      >
                        <WarningIcon
                          color="warning"
                          sx={{
                            fontSize: "1em",
                            marginRight: "8px",
                            verticalAlign: "middle",
                          }}
                        />
                      </Tooltip>
                    ) : opponentAvailableGames.length > 0 &&
                      !opponentAvailableGames.some(
                        (g) =>
                          selectedSave != null &&
                          g.rom == selectedSave.romName &&
                          g.patch == null
                      ) ? (
                      <Tooltip title={<Trans i18nKey="play:no-remote-copy" />}>
                        <WarningIcon
                          color="warning"
                          sx={{
                            fontSize: "1em",
                            marginRight: "8px",
                            verticalAlign: "middle",
                          }}
                        />
                      </Tooltip>
                    ) : null}{" "}
                    <Trans i18nKey="play:unpatched" />
                  </MenuItem>
                  {eligiblePatchNames.map((patchName) => {
                    const v = JSON.stringify(patchName);
                    return (
                      <MenuItem key={v} value={v}>
                        <ListItemText
                          primary={
                            <>
                              {opponentSettings?.gameInfo != null &&
                              !Object.keys(patches[patchName].versions)
                                .map(
                                  (v) =>
                                    patches[patchName].versions[v]
                                      .netplayCompatibility
                                )
                                .some(
                                  (nc) =>
                                    nc ==
                                    getNetplayCompatibility(
                                      opponentSettings!.gameInfo!
                                    )
                                ) ? (
                                <Tooltip
                                  title={
                                    <Trans i18nKey="play:incompatible-game" />
                                  }
                                >
                                  <WarningIcon
                                    color="warning"
                                    sx={{
                                      fontSize: "1em",
                                      marginRight: "8px",
                                      verticalAlign: "middle",
                                    }}
                                  />
                                </Tooltip>
                              ) : opponentAvailableGames.length > 0 &&
                                !opponentAvailableGames.some(
                                  (g) =>
                                    selectedSave != null &&
                                    g.rom == selectedSave.romName &&
                                    g.patch != null &&
                                    g.patch.name == patchName
                                ) ? (
                                <Tooltip
                                  title={
                                    <Trans i18nKey="play:no-remote-copy" />
                                  }
                                >
                                  <WarningIcon
                                    color="warning"
                                    sx={{
                                      fontSize: "1em",
                                      marginRight: "8px",
                                      verticalAlign: "middle",
                                    }}
                                  />
                                </Tooltip>
                              ) : null}{" "}
                              {patches[patchName].title}
                            </>
                          }
                          secondary={
                            <Trans
                              i18nKey="play:patch-byline"
                              values={{
                                authors: listFormatter.format(
                                  patches[patchName].authors.flatMap(
                                    ({ name }) => (name != null ? [name] : [])
                                  )
                                ),
                              }}
                            />
                          }
                        />
                      </MenuItem>
                    );
                  })}
                </Select>
              </FormControl>
              <FormControl fullWidth size="small">
                <InputLabel id="patch-version-label">
                  <Trans i18nKey="play:patch-version" />
                </InputLabel>
                <Select
                  labelId="patch-version-label"
                  disabled={
                    selectedSave == null || patchName == null || battleReady
                  }
                  size="small"
                  value={patchVersion || ""}
                  label={<Trans i18nKey="play:patch-version" />}
                  onChange={(e) => {
                    setPatchVersion(e.target.value);
                  }}
                  fullWidth
                >
                  {patchVersions != null
                    ? patchVersions.map((version) => {
                        return (
                          <MenuItem key={version} value={version}>
                            {opponentSettings?.gameInfo != null &&
                            patchName != null &&
                            patchVersion != null &&
                            patches[patchName].versions[patchVersion]
                              .netplayCompatibility !=
                              getNetplayCompatibility(
                                opponentSettings!.gameInfo!
                              ) ? (
                              <Tooltip
                                title={
                                  <Trans i18nKey="play:incompatible-game" />
                                }
                              >
                                <WarningIcon
                                  color="warning"
                                  sx={{
                                    fontSize: "1em",
                                    marginRight: "8px",
                                    verticalAlign: "middle",
                                  }}
                                />
                              </Tooltip>
                            ) : opponentAvailableGames.length > 0 &&
                              !opponentAvailableGames.some(
                                (g) =>
                                  selectedSave != null &&
                                  g.rom == selectedSave.romName &&
                                  g.patch != null &&
                                  g.patch.name == patchName &&
                                  g.patch.version == version
                              ) ? (
                              <Tooltip
                                title={<Trans i18nKey="play:no-remote-copy" />}
                              >
                                <WarningIcon
                                  color="warning"
                                  sx={{
                                    fontSize: "1em",
                                    marginRight: "8px",
                                    verticalAlign: "middle",
                                  }}
                                />
                              </Tooltip>
                            ) : null}{" "}
                            {version}
                          </MenuItem>
                        );
                      })
                    : []}
                </Select>
              </FormControl>
            </Stack>
          </Collapse>
        </Box>
        {selectedSave != null ? (
          <Stack direction="column" flexGrow={1}>
            <SaveViewerWrapper
              romName={selectedSave.romName}
              filename={selectedSave.saveName}
              incarnation={incarnation}
            />
          </Stack>
        ) : (
          <Box
            flexGrow={1}
            display="flex"
            justifyContent="center"
            alignItems="center"
            sx={{ userSelect: "none", color: "text.disabled" }}
          >
            <Stack alignItems="center" spacing={1}>
              <SportsEsportsOutlinedIcon sx={{ fontSize: "4rem" }} />
              <Typography variant="h6">
                <Trans i18nKey="play:no-save-selected" />
              </Typography>
            </Stack>
          </Box>
        )}
        <BattleStarter
          saveName={selectedSave != null ? selectedSave.saveName : null}
          gameInfo={
            selectedSave != null
              ? {
                  rom: selectedSave.romName,
                  patch:
                    patchVersion != null
                      ? {
                          name: patchName!,
                          version: patchVersion,
                        }
                      : undefined,
                }
              : null
          }
          onExit={() => {
            setIncarnation((incarnation) => incarnation + 1);
          }}
          onReadyChange={(ready) => {
            setBattleReady(ready);
          }}
          onOpponentSettingsChange={(settings) => {
            setOpponentSettings(settings);
          }}
        />
      </Stack>
    </Box>
  );
}
