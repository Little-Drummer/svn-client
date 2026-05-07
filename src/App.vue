<script setup lang="ts">
// import HelloWorld from './components/HelloWorld.vue'
import { invoke } from "@tauri-apps/api/core"

async function test() {

  const result = await invoke("hello", {

    name: "svn"

  })

  console.log(result)

}
</script>

<template>
<!--  <HelloWorld />-->
  <button @click="test">
    test
  </button>
</template>
