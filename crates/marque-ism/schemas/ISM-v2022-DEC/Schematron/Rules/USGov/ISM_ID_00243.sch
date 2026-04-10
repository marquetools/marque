<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00243">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00243][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [RSV],
    then it must also contain a compartment [RSV-XXX].
    
    Human Readable: RESERVE is not permitted as a stand-alone value and a compartment must be expressed.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    If the document is an ISM_USGOV_RESOURCE, for each element which specifies attribute @ism:SCIcontrols 
    with a value containing the token [RSV], this rule ensures that attribute @ism:SCIcontrols is 
    specified with a value containing a token maching the regular expression "RSV-[A-Z0-9]{3}".
    
    If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
    and this rule returns true. If the current element has attribute @ism:SCIcontrols specified
    with a value containing [RSV], then this rule ensures that attribute @ism:SCIcontrols also contains the value [RSV-XXX].
  </sch:p>
  <sch:rule id="ISM-ID-00243-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('RSV'))]">
      <sch:assert test="util:containsAnyTokenMatching(@ism:SCIcontrols, ('RSV-[A-Z0-9]{3}'))" flag="error" role="error">
        [ISM-ID-00243][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [RSV],
        then it must also contain a compartment [RSV-XXX].
        
        Human Readable: RESERVE is not permitted as a stand-alone value and a compartment must be expressed.
    </sch:assert>
  </sch:rule>
</sch:pattern>