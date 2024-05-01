// Top-level load function for the main index page.
async function load() {
  GLOBAL_DATA.currentRun = await getBench("./");
  makeModeSelectors();

  const previousRuns = await getPreviousRuns();
  const initialRunIdx = findBenchToCompareIdx(previousRuns);
  loadBaseline(previousRuns[initialRunIdx].url);

  buildNightlyDropdown("comparison", previousRuns, initialRunIdx);

  refreshView();
}

function selectAllModes(enabled) {
  const checkboxContainer = document.getElementById("modeCheckboxes");
  checkboxContainer.childNodes.forEach((checkbox) => {
    checkbox.checked = enabled;
    enabled
      ? GLOBAL_DATA.enabledModes.add(checkbox.id)
      : GLOBAL_DATA.enabledModes.delete(checkbox.id);
  });
  refreshView();
}

function toggleCheckbox(mode) {
  if (GLOBAL_DATA.enabledModes.has(mode)) {
    GLOBAL_DATA.enabledModes.delete(mode);
  } else {
    GLOBAL_DATA.enabledModes.add(mode);
  }
  refreshView();
}

async function loadBaseline(url) {
  const data = await getBench(url + "/");
  clearWarnings();
  GLOBAL_DATA.baselineRun = data;
  const benchmarkNames = Object.keys(GLOBAL_DATA.currentRun);
  // Add warnings if the baseline run had a benchmark that the current run doesn't
  Object.keys(data).forEach((benchName) => {
    if (!benchmarkNames.includes(benchName)) {
      addWarning(
        `Baseline run had benchmark ${benchName} that the current run doesn't`,
      );
    }
  });
  refreshView();
}

function toggleWarnings() {
  const elt = document.getElementById("warnings-toggle");
  elt.classList.toggle("active");
  const content = elt.nextElementSibling;
  if (content.style.display === "block") {
    elt.innerText = `Show ${GLOBAL_DATA.warnings.size} Warnings`;
    content.style.display = "none";
  } else {
    elt.innerText = `Hide ${GLOBAL_DATA.warnings.size} Warnings`;
    content.style.display = "block";
  }
}
