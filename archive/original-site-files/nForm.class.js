class nForm {
    constructor(form) {
        this.ajaxUrl = document.querySelector('meta[name="ajaxUrl"]').content;
        this.form = form;
        this.obj = {};
        this.exitFlag = false;
        if ('init' in this) this.init();
        if ('bind' in this) this.bind();
    }
    init() {
        this.tooltip();
    }
    bind() {
    }

    tooltip() {
        this.form.find('.tooltip__link').each(function(){
            tippy($(this).get(0), {
                flipBehavior: ["right", "left"],
                theme: 'nform',
                placement: 'right',
                appendTo: 'parent',
                content: $(this).next().html(),
            });
        });
    }
    sa(obj) {
      var a = [];
      for (var k in obj) {
          a.push({name:k,value:obj[k]});
      }
      return(a);
    }

    goto(source) {
        if ('eventBeforeGoto' in this) this.eventBeforeGoto(source);

        var current_slide = source.closest("div.js-step[data-nstep]");
        if (source.attr('data-nstep')) {
            var next_slide = this.form.find("div.js-step[data-nstep='" + source.attr('data-nstep') + "']");
            if ('eventBeforeShow' in this) this.eventBeforeShow(next_slide);
            source.closest("div.js-step[data-nstep]").hide();
            this.form.find("div.js-step[data-nstep='" + source.attr('data-nstep') + "']").show();
            source.closest('.modal__container').animate({scrollTop: 0},100);
        }
        if (source.data('nform')) {
            this.resetForm();
            MicroModal.close(source.closest('.modal').attr('id'));
            MicroModal.show(source.data('nform'));
        }
    }
    getForm() {
        return this.form;
    }
    resetForm() {
        this.form.get(0).reset();
        this.form.find('.form__submit').prop('disabled', false);
        this.showFirstStep();
    }

    formThankYou(id) {
        this.obj.method = 'formThankYou';
        this.obj.formID = id;
        this.send();
    }

    resetCaptcha(target = this.form) {
        target.find('.form__label--captcha > img').click();
        if (target.find('.form__label--captcha > img').length)
        {
          target.find('.form__label--captcha > img').each(function(i){
              $(this).trigger('click');
          });
        }
        target.find('.captcha-input').val('');
    }

    fetchCaptcha(target = this.form) {
        var that = this;
        target.find('.form__label--captcha').each(function(i){
            var el = $(this);
            if (el.find('img').length==0) {
                that.obj.method = 'fetchCaptcha';
                that.send((answer) => {
                    el.html(answer.html);
                });
            }
            else {
                var src=el.find('img').attr('src').split('?');
                el.find('img').removeAttr('src').attr('src',src[0]+'?t='+Date.now());
            }
        });
    }

    showFirstStep() {
        this.form.find("div.js-step[data-nstep]").hide();
        this.form.find("div.js-step.nform_step_start").show();
    }

    send(success, failure) {
        $.post(this.ajaxUrl, this.obj, (answer) => {
            this.obj = {};
            answer.result && typeof success === 'function' ? success(answer) :
                !answer.result && typeof failure === 'function' ? failure(answer) : null;
            this.obj = {};
        }, 'JSON').fail((error)=>{failure(error);});
    };

};
window._forms = {};
window._classes = {};
$(document).ready(function () {
    $('form[data-nclass]').each(function(i,el){
      $class = $(this).data('nclass');
      window._forms[$class] = new window._classes[$class]($(this));
    });
});

$(document).on('click', 'a[data-micromodal-trigger]', function () {
    var $id = $(this).attr('data-micromodal-trigger');
    var modal = $('#'+$id);
    if (modal.hasClass('modal-ajaxload') && !modal.hasClass('modal-loaded')) {
        obj = {method:'modal-ajaxload',ajaxload:modal.data('form')};
        ajax_url = document.querySelector('meta[name="ajaxUrl"]').content;
        $.post(ajax_url, obj, (answer) => {
            if (answer.result) {
                modal.addClass('modal-loaded');
                if (modal.hasClass('modal-preloader'))
                    modal.find('.modal__body').append(answer.html);
                else
                    modal.html(answer.html);
                if (answer.filescript)
                    $.getScript(answer.filescript).done(function(){console.log('filescript load')});
            }
        }, 'JSON');

    }
});

(function($){

  var methods = {
      init : function( options ) {
          var options = $.extend( {
            'minChars' : 3,
            'maxList' : 10,
            'equal' : true,
            'overtext': false,
            'list': []
          }, options);
          var logEvent = function(e,obj,extra='') {
              if (!options.debug) return;
              const c = console;
              var t = (e&&e.target)?e.target:obj[0], add = '';
              if (t.localName!='input') { add = ' > ul > li' + ((t.attributes && t.attributes.rel)?`[rel=${t.attributes.rel.value}]`:''); t = obj[0]; }
              var n = t.localName + ((t.id)?`#${t.id}`:'') + ((t.name)?`[name='${t.name}']`:'');
              if (t.attributes && t.attributes.rel) n += `[rel=${t.attributes.rel.value}]`;
              if (extra) add+=' !'+extra;
              var v = `V:'${obj.val()}' LE:'${obj.data('lastevent')}' S:'${obj.data('strong')}' L:'${obj.data('last')}' O:'${obj.data('old')}'`;
              c.log(new Date().toLocaleTimeString('ua',{timeStyle: "medium"}), n + add, 'E:'+ ((e)?e.type + ((e.namespace)?'.'+e.namespace:''):'*??*'), v);
          }
          return this.each(function(){
              var $this = $(this),
                 data = $this.data('tooltip'),
                 tooltip = $('<div />', {
                   text : $this.attr('title')
                 });

              if ( ! data ) {
                  if (!$(this).parent().hasClass('autoselect'))
                      $(this).wrap('<div class="autoselect" />');
                  if ($(this).next().prop('tagName')!='DIV') {
                      $(this).parent().append($('<div/>',{class:'nform__error_info',style:'display:none;'}));
                  }
              }
              $(this).data('autoselect', {
                target : $this,
                options: options
              }).trigger('autoselect.init');
        }).off('input blur keydown focus').on('input',function(e,extra){
            $('.autoselect .autoselect-focus').removeClass('autoselect-focus');
            $(this).addClass('autoselect-focus');
            $(this).trigger('autoselect.input',extra);
            var $this = $(this), val = $(this).val();
            if (val.length < options.minChars) {
                if ($(this).siblings('ul.autoselect-list').length) $(this).siblings('ul.autoselect-list').css('border','0').remove();
                $this.trigger('autoselect.renderlist',e);
                return false;
            }
            var strong = null;
            $('ul.autoselect-list').each(function(i,el){
                if ($this.attr('rel') && $this.attr('rel')==$(this).siblings('input').attr('rel')) return;
                if ($this.attr('name') && $this.attr('name')==$(this).siblings('input').attr('name')) return;
                $(this).css('border','0').remove();
                $this.trigger('autoselect.renderlist',e);
            });
            if ($(this).val() != $(this).data('last')) strong = false;
            if (Array.isArray(options.list)) {
                if (options.list.length == 0) return;
            }
            else {
                if (Object.keys(options.list) == 0) return;
            }
            var ul = $('<ul/>',{class:'autoselect-list'}).data('parent',$this);
            ul.on('mousedown',function(e){$this.data('lastevent','ul-mousedown');});
            if (Array.isArray(options.list)) {
                for (let i = 0; i < options.list.length; i++) {
                    data_ = options.list[i].toLowerCase();
                    val_ = val.toLowerCase().replace(/[-[\]\<\>{}()*+?,\\^$|#.]/g, "\\$&");
                    var val_reg = new RegExp(val_,'ig');
                    if (data_.match(val_)) {
                        li = $("<div/>",{rel: options.list[i]});
                        li.html(options.list[i].replace(val_reg, "<strong>$&</strong>"));
                        li.on('click',function(e){
                            $this.data('lastevent','click');
                            $(this).parent('ul.autoselect-list').css('border','0').remove();
                            if (options.overtext) {
                                $this.data('old',$this.val());
                                if ($this.data('last')) $this.val($this.data('last'));
                            }
                            else {
                              $this.trigger('autoselect.beforechange');
                            }
                            $this.val($(this).attr('rel')).data('last',options.list[i]).data('strong',true).trigger('autoselect.change');
                        });
                        ul.append(li);
                    }
                    if (options.list[i]==val) strong = true;
                }
            }
            else {
                for (let i in options.list) {
                    data_ = i.toLowerCase();
                    val_ = val.toLowerCase().replace(/[-[\]\<\>{}()*+?,\\^$|#.]/g, "\\$&");
                    var val_reg = new RegExp(val_,'ig');
                    if (data_.match(val_)) {
                        li = $("<div/>",{rel: i});
                        li.html(i.replace(val_reg, "<strong>$&</strong>"));
                        li.on('click',function(e){
                            $this.data('lastevent','click');
                            $(this).parent('ul.autoselect-list').css('border','0').remove();
                            if (options.overtext) {
                                $this.data('old',$this.val());
                                if ($this.data('last')) $this.val($this.data('last'));
                            }
                            else {
                              $this.data('old',$this.val());
                              $this.trigger('autoselect.beforechange');
                            }
                            $this.val($(this).attr('rel')).data('last',i).data('strong',true).trigger('autoselect.change',{key: options.list[i]});
                        });
                        ul.append(li);
                    }
                    if (options.list[i]==val) strong = true;
                }
            }
            if (strong !== null) $(this).data('strong',strong);
            $(this).data('autoselect-list',ul.children().length);
            $(this).siblings('ul.autoselect-list').css('border','0').remove();
            if (ul.children().length) {
                $(this).parent().append(ul);
                var size = 0;
                $(this).siblings('ul.autoselect-list').find('div').each(function(i,el){ if (i<3) size+=$(this).outerHeight();});
                document.documentElement.style.setProperty('--autoselect-list', size + 'px');
                $(this).siblings('ul.autoselect-list').on('scroll',function(e){
                    if ($(this).siblings('input').hasClass('autoselect-focus'))
                        $(this).siblings('.autoselect-focus').removeClass('autoselect-focus');
                });
            }
            $(this).trigger('autoselect.afterinput');
      }).on('blur',function(e){
            var that = $(this);
            setTimeout(function(){
                if (that.data('lastevent')=='ul-mousedown') return;
                if (that.data('lastevent')=='click') return;
                that.siblings('ul.autoselect-list').css('border','0').remove();
                if (options.equal && that.val()!=that.data('last')) that.data('last','').val('');
                if (options.overtext && that.data('lastevent')!='click') {
                    if (that.data('last')==that.val()) {
                        var val = that.val();
                        that.val(that.data('old')).data('last',that.data('old')).data('strong',true);
                        that.data('old',val);
                    }
                    else that.data('strong',false);
                }
                that.trigger('autoselect.blur');
                that.data('lastevent','blur');
            },300);
      }).on('keydown',function(e){
          $(this).data('lastevent',e.type);
          if (e.keyCode == 13) {
              if ($(this).siblings('ul.autoselect-list').children().length==1) {
                  $(this).siblings('ul.autoselect-list').find(':nth-child(1)').click();
              }
              $(this).trigger('blur');
              e.preventDefault();
              return false;
          }
          if (e.keyCode == 27) {
            e.preventDefault();
            return true;
          }
          if (e.keyCode>=33 && e.keyCode<=40) {
              $(this).data('lastevent','focus');
              return true;
          }
          if (e.keyCode==9 && options.overtext) {
              $(this).data('lastevent','focus');
          }
          if (e.keyCode==9 || e.keyCode==13) {
              $(this).siblings('ul.autoselect-list').css('border','0').remove();
          }
          $(this).trigger('autoselect.afterkeydown',e);
      }).on('focus',function(e){
          $(this).data('lastevent',e.type);
          if (options.overtext) {
              if ($(this).data('old')) {
                  var val = $(this).val();
                  $(this).val($(this).data('old')).data('last',$(this).data('old')).data('strong',true);
                  $(this).data('old',val);
              }
          }
          else {
              $(this).data('last',$(this).val());
          }
          $(this).trigger('input',{e:'focus'});
       });
    },
    destroy : function( ) {
        return this.each(function(){
            var $this = $(this),
            data = $this.data('autoselect');
            $(window).unbind('.autoselect');
            $this.removeData();
            if ($(this).next().prop('tagName')=='DIV') {
                $(this).next().hide().remove();
            }
            $(this).siblings('ul.autoselect-list').remove();
            $(this).trigger('autoselect.destroy');
            $(this).unwrap();
        })

     },
     check : function(val) {
          return $(this).data('autoselect-list');
     },
     showoptions : function() {
          console.log($(this).data('autoselect').options);
     },
     update : function( content ) {
        for(var k in content) options[k] = content[k];
     }
  };

  $.fn.autoselect = function( method ) {
    if ( methods[method] ) {
      return methods[method].apply( this, Array.prototype.slice.call( arguments, 1 ));
    } else if ( typeof method === 'object' || ! method ) {
      return methods.init.apply( this, arguments );
    }
  };

})( jQuery );
