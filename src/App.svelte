<script lang="ts">
  import FolderTree from './lib/components/FolderTree.svelte';
  import PdfList from './lib/components/PdfList.svelte';
  import SearchBar from './lib/components/SearchBar.svelte';
  import SearchResults from './lib/components/SearchResults.svelte';
  import PdfViewer from './lib/components/PdfViewer.svelte';
  import { selectedPdfPath } from './lib/stores';
</script>

<main>
  <div class="left-panel">
    <FolderTree />
  </div>
  <div class="center-panel">
    <SearchBar />
    <PdfList />
    <SearchResults />
  </div>
  <div class="right-panel">
    <PdfViewer pdfPath={$selectedPdfPath} />
  </div>
</main>

<style>
  main {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .left-panel {
    width: 220px;
    min-width: 180px;
    border-right: 1px solid #ddd;
    overflow: hidden;
  }

  .center-panel {
    flex: 1;
    min-width: 300px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .right-panel {
    width: 400px;
    min-width: 300px;
    border-left: 1px solid #ddd;
    overflow: hidden;
  }
</style>