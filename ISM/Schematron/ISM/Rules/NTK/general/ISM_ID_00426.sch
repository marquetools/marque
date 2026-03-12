<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00033 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00426" is-a="VocabHasCorrespondingVersion">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00426][Error] An @ntk:sourceVersion must be specified for the built-in datasphere:mn:region vocabulary type.</sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Use the VocabHasCorrespondingVersion abstract pattern to require an ntk:VocabularyType with
      @ntk:sourceVersion specified and @ntk:name = 'datasphere:mn:region'.</sch:p>
   <sch:param name="context" value="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:mn:region']"/>
   <sch:param name="vocab" value="'datasphere:mn:region'"/>
   <sch:param name="errMsg" value="'[ISM-ID-00426][Error]An @ntk:sourceVersion must be specified for the built-in datasphere:mn:region vocabulary type.'"/>
</sch:pattern>
