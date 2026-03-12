<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00041 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00434">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00434][Error] Source versions (@ntk:sourceVersion) must be consistent for all NTK Profiles 
      within a document that contribute to the actual overall access restrictions of the document.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For any given ntk:VocabularyType that is tied to a CES, determine how many distinct versions of that
      CES are specified. Report an error if there is more than one version found.
   </sch:p>
  <sch:rule abstract="true" id="abs_rule_00041">
     <sch:let name="versions" value="distinct-values(for $version in //ntk:Access//ntk:VocabularyType[@ntk:name=$vocab]/@ntk:sourceVersion return $version)"/>
     <sch:assert test="not(count($versions)&gt;1)" flag="error" role="error">
        [ISM-ID-00434][Error] Source versions (@ntk:sourceVersion) must be consistent for all NTK Profiles
        within a document that contribute to the actual overall access restrictions of the document.
        Found <sch:value-of select="$vocab"/> versions: <sch:value-of select="$versions"/> 
     </sch:assert>
  </sch:rule>
   <sch:rule id="ISM-ID-00434-R2" context="ntk:Access//ntk:VocabularyType[@ntk:name='datasphere:mn:region']">
      <sch:let name="vocab" value="'datasphere:mn:region'"/>
      <sch:extends rule="abs_rule_00041"/>
   </sch:rule>
   <sch:rule id="ISM-ID-00434-R3" context="ntk:Access//ntk:VocabularyType[@ntk:name='datasphere:mn:issue']">
      <sch:let name="vocab" value="'datasphere:mn:issue'"/>
      <sch:extends rule="abs_rule_00041"/>
   </sch:rule>
   <sch:rule id="ISM-ID-00434-R4" context="ntk:Access//ntk:VocabularyType[@ntk:name='organization:usa-agency']">
      <sch:let name="vocab" value="'organization:usa-agency'"/>
      <sch:extends rule="abs_rule_00041"/>
   </sch:rule>
   <sch:rule id="ISM-ID-00434-R5" context="ntk:Access//ntk:VocabularyType[@ntk:name='datasphere:license']">
      <sch:let name="vocab" value="'datasphere:license'"/>
      <sch:extends rule="abs_rule_00041"/>
   </sch:rule>
</sch:pattern>
