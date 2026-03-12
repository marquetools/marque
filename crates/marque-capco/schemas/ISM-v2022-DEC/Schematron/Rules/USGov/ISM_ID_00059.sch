<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00059">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00059][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification of ISM_RESOURCE_ELEMENT 
        has a value of [S] then no element meeting ISM_CONTRIBUTES_USA in the document may have 
        a @ism:classification attribute of [TS].
        
        Human Readable: USA SECRET documents can't have TOP SECRET data.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE and attribute @ism:classification
      on $ISM_RESOURCE_ELEMENT has a value of [S], this rule ensures that
      no element meeting ISM_CONTRIBUTES_USA has attribute @ism:classification with
      value [TS].
    </sch:p>
    <sch:rule id="ISM-ID-00059-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and normalize-space(string(@ism:classification))='S']">
      <sch:assert test="every $ele in $partTags satisfies not(util:containsAnyOfTheTokens($ele/@ism:classification, ('TS')))" flag="error" role="error">
          [ISM-ID-00059][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification of ISM_RESOURCE_ELEMENT 
          has a value of [S] then no element meeting ISM_CONTRIBUTES_USA in the document may have 
          a @ism:classification attribute of [TS].
          
          Human Readable: USA SECRET documents can't have TOP SECRET data.
        </sch:assert>
    </sch:rule>
</sch:pattern>