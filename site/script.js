const copyButton = document.querySelector("[data-copy]");

if (copyButton) {
  copyButton.addEventListener("click", async () => {
    const value = copyButton.getAttribute("data-copy");
    try {
      await navigator.clipboard.writeText(value);
      copyButton.textContent = "Copied";
      setTimeout(() => {
        copyButton.textContent = "Copy install";
      }, 1600);
    } catch {
      copyButton.textContent = value;
    }
  });
}
