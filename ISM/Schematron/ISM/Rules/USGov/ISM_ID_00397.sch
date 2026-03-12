<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00397">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00397][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a name token that complies with the pattern [KLM-] followed by any alphanumeric string, then attribute
        @ism:disseminationControls must contain the name token [OC], except for the [KLM-R] compartment which does not require [OC].
        
        Human Readable: A USA document containing a KLM compartment data must be marked for ORIGINATOR CONTROLLED (ORCON)
        dissemination, except for the KLM-R compartment which does not require ORCON dissemination.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token
        starting with [KLM-], this rule ensures that attribute
        @ism:disseminationControls is specified with a value containing the
        token [OC]. The one exception to the requirement for [OC] is the [KLM-R] compartment.
    </sch:p>
    <sch:rule id="ISM-ID-00397-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-[A-Z0-9]*$')) and not(util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R$')))]">
      <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))" flag="error" role="error">
          [ISM-ID-00397][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
          contains a name token that complies with the pattern [KLM-] followed by any alphanumeric string, then attribute
          @ism:disseminationControls must contain the name token [OC], except for the [KLM-R] compartment which does not require [OC].
          
          Human Readable: A USA document containing a KLM compartment data must be marked for ORIGINATOR CONTROLLED (ORCON)
          dissemination, except for the KLM-R compartment which does not require ORCON dissemination.
        </sch:assert>
    </sch:rule>
</sch:pattern>