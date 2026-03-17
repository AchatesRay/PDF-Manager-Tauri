<script lang="ts">
  import FolderTree from './lib/components/FolderTree.svelte';
  import PdfList from './lib/components/PdfList.svelte';
  import SearchBar from './lib/components/SearchBar.svelte';
  import SearchResults from './lib/components/SearchResults.svelte';
</script>

<main>
  <FolderTree />
  <div class="content">
    <SearchBar />
    <PdfList />
    <SearchResults />
  </div>
</main>

<style>
  main {
    display: flex;
    height: 100vh;
    width: 100vw;
  }

  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
</style>