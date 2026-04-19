<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00353" is-a="NtkHasCorrespondingData">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00353][Error] Banner or portion that contributes to roll-up must contain [OC] if ORCON NTK metadata exists.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Invokes abstract rule NtkHasCorrespondingData.
   </sch:p>
   <sch:param name="ruleId" value="'ISM-ID-00353'"/>
   <sch:param name="policyName" value="'ORCON'"/>
   <sch:param name="uriPrefix" value="'urn:us:gov:ic:aces:ntk:oc'"/>
   <sch:param name="attr" value="'disseminationControls'"/>
   <sch:param name="dataType" value="'OC'"/>
   <sch:param name="dataTokenList" value="$partDisseminationControls_tok"/>
   <sch:param name="bannerTokenList" value="$bannerDisseminationControls_tok"/>
</sch:pattern>