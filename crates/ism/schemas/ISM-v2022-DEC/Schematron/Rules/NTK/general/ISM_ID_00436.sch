<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00043 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00436">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00436][Warning] The source version (@ntk:sourceVersion) must match the version of the CVE being used
      to validate values of the NTK instance.</sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For any given ntk:VocabularyType that is tied to a CES, check the claimed sourceVersion against
      the version of the CVE file being used for validation and ensure they are equal.</sch:p>
  <sch:rule abstract="true" id="abs_rule_00043">
     <sch:assert test="$cve/@specVersion = @ntk:sourceVersion" flag="warning" role="error">
        [ISM-ID-00436][Warning] The source version (@ntk:sourceVersion) must match the version of the CVE
        being used to validate values of the NTK instance.
        The NTK claims that the vocabulary <sch:value-of select="@ntk:name"/> is compliant with 
        <sch:value-of select="@ntk:sourceVersion"/>, but the CVE used points at spec version 
        <sch:value-of select="$cve/@specVersion"/>.
     </sch:assert>
  </sch:rule>
   <sch:rule id="ISM-ID-00436-R2" context="ntk:Access//ntk:VocabularyType[@ntk:name='datasphere:mn:region']">
      <sch:let name="cve" value="document('../../CVE/MN/CVEnumMNRegion.xml')//cve:CVE"/>
      <sch:extends rule="abs_rule_00043"/>
   </sch:rule>
   <sch:rule id="ISM-ID-00436-R3" context="ntk:Access//ntk:VocabularyType[@ntk:name='datasphere:mn:issue']">
      <sch:let name="cve" value="document('../../CVE/MN/CVEnumMNIssue.xml')//cve:CVE"/>
      <sch:extends rule="abs_rule_00043"/>
   </sch:rule>
   <sch:rule id="ISM-ID-00436-R4" context="ntk:Access//ntk:VocabularyType[@ntk:name='datasphere:license']">
      <sch:let name="cve" value="document('../../CVE/LIC/CVEnumLicLicense.xml')//cve:CVE"/>
      <sch:extends rule="abs_rule_00043"/>
   </sch:rule>
   <sch:rule id="ISM-ID-00436-R5" context="ntk:Access//ntk:VocabularyType[@ntk:name='organization:usa-agency']">
      <sch:let name="cve" value="document('../../CVE/USAgency/CVEnumUSAgencyAcronym.xml')//cve:CVE"/>
      <sch:extends rule="abs_rule_00043"/>
   </sch:rule>
</sch:pattern>
