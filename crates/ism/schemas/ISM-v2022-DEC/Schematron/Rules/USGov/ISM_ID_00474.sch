<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00474">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00474][Warning] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
      contains the name token [HCS], then attribute @ism:SCIcontrols MUST include one of the tokens [HCS-O], [HCS-P] or [HCS-X].
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, then for any element that has attribute @ism:SCIcontrols 
      containing the name token [HCS], the element MUST have @ism:SCIcontrols containing have one of: [HCS-O], [HCS-P] or [HCS-X].
   </sch:p>
   <sch:rule id="ISM-ID-00474-R1" context="*[$ISM_USGOV_RESOURCE and (util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS')))]">
      <sch:assert test="util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O','HCS-P','HCS-X'))" flag="warning" role="warning">
         [ISM-ID-00474][Warning] HCS information requires one of the HCS compartments: [HCS-O], [HCS-P] or [HCS-X]. 
         There are special exemption cases outlined in the IC Markings Register and Manual. Data marked HCS without 
         a compartment and unable to be positively determined to be O, P, or X MUST NOT be shared with entities who do 
         not have all three HCS compartments. Seek your Information System Security Manager's (ISSM’s) guidance. 
      </sch:assert>
   </sch:rule>
</sch:pattern>
