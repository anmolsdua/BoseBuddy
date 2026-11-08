// Update these two values when the release or donation page changes.
const VERSION = "1.1.4";
const DOWNLOAD_URL = `https://github.com/anmolsdua/BoseBuddy/releases/download/v${VERSION}/BoseBuddy-${VERSION}.dmg`;
const DONATE_URL = "https://www.buymeacoffee.com/anmolsdua";

const dialog = document.getElementById("downloadDialog");
const confirmButton = document.getElementById("confirmDownload");
const customInput = document.getElementById("customAmount");
const versionLabel = document.getElementById("dialogVersion");
const chips = [...document.querySelectorAll("[data-amount]")];

let amount = 0;

versionLabel.textContent = `v${VERSION}`;

// Keep every support link on the page in sync with DONATE_URL.
document.querySelectorAll("[data-support]").forEach((link) => {
  link.href = DONATE_URL;
});

function setAmount(value) {
  amount = Math.max(0, Number(value) || 0);
  chips.forEach((chip) => {
    chip.classList.toggle("is-active", Number(chip.dataset.amount) === amount);
  });
  if (amount === 0) {
    confirmButton.textContent = "Download free";
  } else {
    confirmButton.textContent = `Download & donate $${amount}`;
  }
}

function openDialog(event) {
  if (event) event.preventDefault();
  dialog.showModal();
  document.body.style.overflow = "hidden";
}

function closeDialog() {
  dialog.close();
  document.body.style.overflow = "";
}

document.querySelectorAll("[data-download]").forEach((button) => {
  button.addEventListener("click", openDialog);
});

document.querySelectorAll("[data-close]").forEach((button) => {
  button.addEventListener("click", closeDialog);
});

chips.forEach((chip) => {
  chip.addEventListener("click", () => {
    customInput.value = "";
    setAmount(chip.dataset.amount);
  });
});

customInput.addEventListener("input", () => {
  chips.forEach((chip) => chip.classList.remove("is-active"));
  const value = Math.max(0, Number(customInput.value) || 0);
  amount = value;
  confirmButton.textContent = value === 0 ? "Download free" : `Download & donate $${value}`;
});

dialog.addEventListener("click", (event) => {
  if (event.target === dialog) closeDialog();
});

dialog.addEventListener("close", () => {
  document.body.style.overflow = "";
});

confirmButton.addEventListener("click", () => {
  if (amount > 0) {
    const donation = new URL(DONATE_URL);
    donation.searchParams.set("amount", String(amount));
    window.open(donation.toString(), "_blank", "noopener");
  }

  const link = document.createElement("a");
  link.href = DOWNLOAD_URL;
  link.download = "";
  link.rel = "noopener";
  document.body.appendChild(link);
  link.click();
  link.remove();

  closeDialog();
});

// A shareable link that opens the download dialog directly.
if (window.location.hash === "#download") {
  openDialog();
}

setAmount(0);
