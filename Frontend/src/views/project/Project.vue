<template>
  <div>
    <h1 class="text">{{pid}}</h1>
    <div v-if="scripts.length > 0" class="uk-card uk-card-default uk-card-body" v-motion :initial="{
      opacity: 0,
      x: 50,
    }" :enter="{
        opacity: 1,
        x: 0,
      }">
      <ul class="uk-list uk-list-divider script-list">
        <li v-for="script in scripts" :key="script">
          <router-link :to="{
            name: 'Script',
            params: { pid: pid, id: script },
          }">
            {{ script }}
          </router-link>
        </li>
      </ul>
    </div>
  </div>
</template>

<script>

export default {
  name: "Project",
  data() {
    return {
      scripts: []
    }
  },
  props: ["pid", "deletedProject"],
  watch: {
    deletedProject: function () {
      if (this.pid == this.deletedProject.id) {
        this.$router.replace({ name: "Home" });
      }
    },
  },
  methods: {
    getScripts() {
      fetch(`/api/master/project/${this.pid}`)
        .then((data) => data.json())
        .then((data) => {
          if (data.success) {
            this.scripts = data.content.scripts;
          }
        })
        .catch();
    }
  },
  mounted() {
    console.log(this.scripts);
  },
  created() {
    this.getScripts();
  }
};
</script>

<style>

</style>

<!-- <div v-for="script in scripts" :key="script" v-motion :initial="{
  opacity: 0,
  x: 50,
}" :enter="{
  opacity: 1,
  x: 0,
}">
  <div class="uk-card uk-card-default uk-width-auto@m uk-card-body">
    {{ ScriptVue }}
  </div>
</div> -->