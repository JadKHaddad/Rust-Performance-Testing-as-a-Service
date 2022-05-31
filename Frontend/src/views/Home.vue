<template>
  <div>
    <button
      class="uk-button uk-button-default uk-margin-small-right"
      type="button"
      uk-toggle="target: #upload-modal"
      @click="uploadMessage = ''"
    >
      Upload
    </button>

    <div id="upload-modal" uk-modal ref="upload-modal">
      <div class="uk-modal-dialog uk-modal-body">
        <form>
          <div class="uk-margin" uk-margin>
            <h2>Upload a new project</h2>
            <h5>Please make sure all names don't include blank spaces</h5>
            <div class="upload-container">
              <div uk-form-custom="target: true">
                <input type="file" webkitdirectory mozdirectory ref="files" />
                <input
                  class="uk-input uk-form-width-medium"
                  type="text"
                  placeholder="Select project"
                  disabled
                />
              </div>
              <button
                type="button"
                class="uk-button uk-button-default"
                @click="upload"
              >
                Upload
              </button>
              <div v-if="uploading" uk-spinner class="upload-spinner"></div>
            </div>
            <h5>{{ uploadMessage }}</h5>
          </div>
        </form>
      </div>
    </div>

    <h1>Projects</h1>
    <ul class="uk-list">
      <li
        v-for="project in projects"
        :key="project.id"
        v-motion
        :initial="{
          opacity: 0,
          x: 50,
        }"
        :enter="{
          opacity: 1,
          x: 0,
        }"
      >
        <div class="uk-card uk-card-default uk-card-body">
          <h3 class="uk-card-title">{{ project.id }}</h3>
          <ul class="uk-list uk-list-divider script-list">
            <li v-for="script in project.scripts" :key="script">
              <router-link
                :to="{
                  name: 'Script',
                  params: { pid: project.id, id: script },
                }"
              >
                {{ script }}
              </router-link>
            </li>
          </ul>
        </div>
      </li>
    </ul>
    <br />
    <br />
    <label class="checkbox-label">
      <input type="checkbox" class="checkbox-input" />
      <span class="checkbox"> </span>
    </label>
  </div>
</template>

<script>
export default {
  name: "Home",
  props: ["newProject"],
  watch: {
    newProject: function () {
      this.projects.push(this.newProject);
    },
  },
  data() {
    return {
      uploading: false,
      projects: [],
      uploadMessage: "",
    };
  },
  methods: {
    getProjects() {
      fetch("/api/master/projects")
        .then((data) => data.json())
        .then((data) => {
          this.projects = data.content.projects;
          console.log(data);
        })
        .catch();
    },
    hideUploadModal() {
      UIkit.modal(this.$refs["upload-modal"]).hide();
    },
    upload() {
      const files = this.$refs.files.files;
      console.log(files);
      if (files.length < 1) {
        this.uploadMessage = "Please select a directory to upload";
        return false;
      }
      var data = new FormData();
      for (var i = 0; i < files.length; i++) {
        data.append("file" + i, files[i]);
      }
      this.uploading = true;

      // axios
      //   .request({
      //     method: "post",
      //     url: "/api/upload",
      //     data: data,
      //     onUploadProgress: (progressEvent) => {
      //       this.percentCompleted = Math.round(
      //         (progressEvent.loaded * 100) / progressEvent.total
      //       );
      //     },
      //   })
      //   .then((response) => {
      //     this.uploading = false;
      //     const data = response.data;
      //     this.uploadResponse = data;
      //     console.log(data);
      //     if (data.success) {
      //     } else {
      //     }
      //   })
      //   .catch(() => {
      //     this.uploading = false;
      //     console.log("Connection error");
      //   });

      fetch("/api/master/upload", {
        method: "POST",
        body: data,
      })
        .then((data) => data.json())
        .then((data) => {
          console.log(data);
          this.uploading = false;
          if (data.success) {
            this.hideUploadModal();
          } else {
            this.uploadMessage = data.error;
          }
        })
        .catch(() => {
          this.uploading = false;
          this.hideUploadModal();
        });
      return false;
    },
  },
  created() {
    this.getProjects();
  },
};
</script>
