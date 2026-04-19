<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00055 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00454" is-a="VocabHasCorrespondingVersion">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00454][Error] An @ntk:sourceVersion must be specified for the built-in datasphere:rac vocabulary type.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Use the VocabHasCorrespondingVersion abstract pattern to require an ntk:VocabularyType with
      @ntk:sourceVersion specified and @ntk:name = 'datasphere:rac'.</sch:p>
   <sch:param name="context" value="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:rac']"/>
   <sch:param name="vocab" value="'datasphere:rac'"/>
   <sch:param name="errMsg" value="'[ISM-ID-00454][Error]An @ntk:sourceVersion must be specified for the built-in datasphere:rac vocabulary type.'"/>
</sch:pattern>
